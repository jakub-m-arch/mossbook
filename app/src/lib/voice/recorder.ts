// Recording a voice note (E-18, D-21): microphone → AudioWorklet → WebCodecs
// Opus → Ogg. One format on every platform: the browsers' own MediaRecorder
// gives WebM on some, MP4 on others, and WebKitGTK's MP4 does not play on
// macOS (spike for D-21). Nothing touches the disk; the bytes go to the core.

import { OPUS_RATE, muxOggOpus, opusPacketSamples, preSkipFrom, type OpusPacket } from './ogg';

/** Longest recording (D-21); the core refuses anything longer. */
export const MAX_RECORDING_MS = 5 * 60 * 1000;
const MAX_SAMPLES = (MAX_RECORDING_MS / 1000) * OPUS_RATE;
/** 20 ms frames, the Opus default. */
const FRAME = 960;
const BITRATE = 32_000;

// Minimal typings: WebCodecs audio types are not in every TypeScript lib yet.
interface EncodedChunk {
	byteLength: number;
	duration: number | null;
	copyTo(destination: Uint8Array): void;
}
interface EncoderMetadata {
	decoderConfig?: { description?: BufferSource };
}
interface AudioEncoderLike {
	configure(config: object): void;
	encode(data: unknown): void;
	flush(): Promise<void>;
	close(): void;
	readonly state: string;
}
type AudioEncoderCtor = new (init: {
	output: (chunk: EncodedChunk, meta?: EncoderMetadata) => void;
	error: (e: unknown) => void;
}) => AudioEncoderLike;
type AudioDataCtor = new (init: object) => { close(): void };

const globals = globalThis as unknown as {
	AudioEncoder?: AudioEncoderCtor & {
		isConfigSupported(config: object): Promise<{ supported?: boolean }>;
	};
	AudioData?: AudioDataCtor;
};

const CONFIG = { codec: 'opus', sampleRate: OPUS_RATE, numberOfChannels: 1, bitrate: BITRATE };

/** Whether this web view can record voice notes. */
export async function canRecord(): Promise<boolean> {
	if (!globals.AudioEncoder || !globals.AudioData) return false;
	if (typeof AudioWorkletNode === 'undefined') return false;
	if (typeof navigator.mediaDevices?.getUserMedia !== 'function') return false;
	try {
		return !!(await globals.AudioEncoder.isConfigSupported(CONFIG)).supported;
	} catch {
		return false;
	}
}

/** Why a recording could not start, for the message shown to the user. */
export type StartError = 'denied' | 'no_microphone' | 'unsupported' | 'failed';

export function startError(e: unknown): StartError {
	const name = e instanceof DOMException ? e.name : '';
	if (name === 'NotAllowedError' || name === 'SecurityError') return 'denied';
	if (name === 'NotFoundError' || name === 'OverconstrainedError') return 'no_microphone';
	return 'failed';
}

export class VoiceRecorder {
	private packets: OpusPacket[] = [];
	private preSkip: number | undefined;
	private frame = new Float32Array(FRAME);
	private filled = 0;
	private samples = 0;
	private failure: unknown = null;
	private stopped = false;

	private constructor(
		private stream: MediaStream,
		private context: AudioContext,
		private node: AudioWorkletNode,
		private encoder: AudioEncoderLike,
		private onLimit: () => void
	) {}

	/**
	 * Asks for the microphone and starts recording. `onLimit` runs once when
	 * five minutes are reached; the caller then calls `stop()`.
	 */
	static async start(onLimit: () => void): Promise<VoiceRecorder> {
		const Encoder = globals.AudioEncoder;
		if (!Encoder || !globals.AudioData) throw new DOMException('no encoder', 'NotSupportedError');
		const stream = await navigator.mediaDevices.getUserMedia({
			audio: {
				channelCount: 1,
				echoCancellation: true,
				noiseSuppression: true,
				autoGainControl: true
			}
		});
		try {
			// The context resamples the microphone to the Opus rate.
			const context = new AudioContext({ sampleRate: OPUS_RATE });
			await context.audioWorklet.addModule('/voice-capture.js');
			const source = context.createMediaStreamSource(stream);
			const node = new AudioWorkletNode(context, 'mossbook-voice-capture');
			// A silent sink keeps the graph running without playing anything back.
			const mute = context.createGain();
			mute.gain.value = 0;
			source.connect(node).connect(mute).connect(context.destination);

			// The encoder's callbacks need the recorder, which needs the encoder.
			const self: { recorder?: VoiceRecorder } = {};
			const encoder = new Encoder({
				output: (chunk, meta) => self.recorder?.collect(chunk, meta),
				error: (e) => {
					if (self.recorder) self.recorder.failure = e;
				}
			});
			encoder.configure(CONFIG);
			const recorder = new VoiceRecorder(stream, context, node, encoder, onLimit);
			self.recorder = recorder;
			node.port.onmessage = (e: MessageEvent<Float32Array>) => recorder.push(e.data);
			if (context.state === 'suspended') await context.resume();
			return recorder;
		} catch (e) {
			stream.getTracks().forEach((t) => t.stop());
			throw e;
		}
	}

	/** Milliseconds recorded so far. */
	get elapsedMs(): number {
		return Math.round((this.samples / OPUS_RATE) * 1000);
	}

	private collect(chunk: EncodedChunk, meta?: EncoderMetadata) {
		this.preSkip ??= meta?.decoderConfig ? preSkipFrom(meta.decoderConfig.description) : undefined;
		const data = new Uint8Array(chunk.byteLength);
		chunk.copyTo(data);
		const samples =
			opusPacketSamples(data) ??
			(chunk.duration ? Math.round((chunk.duration * OPUS_RATE) / 1e6) : FRAME);
		this.packets.push({ data, samples });
	}

	private push(input: Float32Array) {
		if (this.stopped) return;
		let offset = 0;
		while (offset < input.length && this.samples < MAX_SAMPLES) {
			const n = Math.min(FRAME - this.filled, input.length - offset, MAX_SAMPLES - this.samples);
			this.frame.set(input.subarray(offset, offset + n), this.filled);
			this.filled += n;
			this.samples += n;
			offset += n;
			if (this.filled === FRAME) this.encodeFrame();
		}
		if (this.samples >= MAX_SAMPLES) {
			this.stopped = true;
			this.onLimit();
		}
	}

	private encodeFrame() {
		const AudioDataImpl = globals.AudioData as AudioDataCtor;
		const data = new AudioDataImpl({
			format: 'f32',
			sampleRate: OPUS_RATE,
			numberOfFrames: FRAME,
			numberOfChannels: 1,
			timestamp: Math.round(((this.samples - this.filled) / OPUS_RATE) * 1e6),
			data: this.frame
		});
		this.encoder.encode(data);
		data.close();
		this.frame = new Float32Array(FRAME);
		this.filled = 0;
	}

	private release() {
		this.node.port.onmessage = null;
		this.node.disconnect();
		this.stream.getTracks().forEach((t) => t.stop());
		void this.context.close().catch(() => {});
	}

	/** Stops and returns the Ogg Opus file. */
	async stop(): Promise<Uint8Array<ArrayBuffer>> {
		this.stopped = true;
		this.release();
		const total = this.samples;
		// The last, partial frame is padded with silence; the granule trims it.
		if (this.filled > 0) this.encodeFrame();
		await this.encoder.flush();
		this.encoder.close();
		if (this.failure) throw this.failure;
		const file = muxOggOpus(this.packets, {
			preSkip: this.preSkip ?? preSkipFrom(undefined),
			channels: 1,
			inputRate: OPUS_RATE,
			totalSamples: total,
			serial: crypto.getRandomValues(new Uint32Array(1))[0]
		});
		this.packets = [];
		return file;
	}

	/** Stops and throws the recording away (e.g. the journal locked). */
	cancel() {
		if (this.encoder.state !== 'closed') this.encoder.close();
		this.stopped = true;
		this.release();
		this.packets = [];
	}
}

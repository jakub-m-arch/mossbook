// Ogg Opus container for voice notes (E-18, D-21): RFC 3533 pages carrying an
// RFC 7845 stream. The core checks the result (`inspect_ogg_opus`) and takes
// the length from the last granule position.

/** Opus counts granule positions at 48 kHz (RFC 7845 §4). */
export const OPUS_RATE = 48_000;
/** libopus's look-ahead at 48 kHz, used when the encoder does not say. */
export const DEFAULT_PRE_SKIP = 312;
/** Packets per page: about one second of 20 ms frames keeps overhead low. */
const PACKETS_PER_PAGE = 50;

export interface OpusPacket {
	data: Uint8Array;
	/** Samples at 48 kHz this packet decodes to. */
	samples: number;
}

export interface OggOpusOptions {
	preSkip: number;
	channels: number;
	/** The rate the audio was captured at (informational in the header). */
	inputRate: number;
	/** Samples of real audio; the last packet's padding beyond it is trimmed. */
	totalSamples: number;
	serial: number;
}

const CRC_TABLE = (() => {
	const table = new Uint32Array(256);
	for (let i = 0; i < 256; i++) {
		let r = i << 24;
		for (let bit = 0; bit < 8; bit++) r = r & 0x80000000 ? (r << 1) ^ 0x04c11db7 : r << 1;
		table[i] = r >>> 0;
	}
	return table;
})();

/** RFC 3533 page checksum: CRC-32, polynomial 0x04c11db7, no reflection. */
export function oggCrc(bytes: Uint8Array): number {
	let crc = 0;
	for (const b of bytes) crc = ((crc << 8) ^ CRC_TABLE[((crc >>> 24) ^ b) & 0xff]) >>> 0;
	return crc;
}

const FLAG_FIRST = 0x02;
const FLAG_LAST = 0x04;

function page(
	packets: Uint8Array[],
	granule: bigint,
	serial: number,
	sequence: number,
	flags: number
): Uint8Array {
	const lacing: number[] = [];
	for (const p of packets) {
		let n = p.length;
		while (n >= 255) {
			lacing.push(255);
			n -= 255;
		}
		lacing.push(n);
	}
	if (lacing.length > 255) throw new Error('too many segments for one page');
	const bodyLength = packets.reduce((n, p) => n + p.length, 0);
	const out = new Uint8Array(27 + lacing.length + bodyLength);
	const view = new DataView(out.buffer);
	out.set([0x4f, 0x67, 0x67, 0x53]); // "OggS"
	out[5] = flags;
	view.setBigUint64(6, granule, true);
	view.setUint32(14, serial, true);
	view.setUint32(18, sequence, true);
	out[26] = lacing.length;
	out.set(lacing, 27);
	let offset = 27 + lacing.length;
	for (const p of packets) {
		out.set(p, offset);
		offset += p.length;
	}
	view.setUint32(22, oggCrc(out), true);
	return out;
}

function opusHead(o: OggOpusOptions): Uint8Array {
	const head = new Uint8Array(19);
	const view = new DataView(head.buffer);
	head.set(new TextEncoder().encode('OpusHead'));
	head[8] = 1; // version
	head[9] = o.channels;
	view.setUint16(10, o.preSkip, true);
	view.setUint32(12, o.inputRate, true);
	// Output gain 0, channel mapping family 0.
	return head;
}

/** No metadata beyond a vendor string: no dates, devices or names. */
function opusTags(): Uint8Array {
	const vendor = new TextEncoder().encode('Mossbook');
	const tags = new Uint8Array(8 + 4 + vendor.length + 4);
	const view = new DataView(tags.buffer);
	tags.set(new TextEncoder().encode('OpusTags'));
	view.setUint32(8, vendor.length, true);
	tags.set(vendor, 12);
	return tags;
}

/** Wraps encoded Opus packets into a complete Ogg Opus file. */
export function muxOggOpus(packets: OpusPacket[], o: OggOpusOptions): Uint8Array<ArrayBuffer> {
	if (packets.length === 0) throw new Error('no audio');
	const pages = [
		page([opusHead(o)], 0n, o.serial, 0, FLAG_FIRST),
		page([opusTags()], 0n, o.serial, 1, 0)
	];
	let samples = 0;
	for (let i = 0; i < packets.length; i += PACKETS_PER_PAGE) {
		const group = packets.slice(i, i + PACKETS_PER_PAGE);
		samples += group.reduce((n, p) => n + p.samples, 0);
		const last = i + PACKETS_PER_PAGE >= packets.length;
		// The last page ends at the real audio, trimming the final frame's padding.
		const end = last ? Math.min(samples, o.totalSamples) : samples;
		pages.push(
			page(
				group.map((p) => p.data),
				BigInt(o.preSkip + end),
				o.serial,
				pages.length,
				last ? FLAG_LAST : 0
			)
		);
	}
	const out = new Uint8Array(pages.reduce((n, p) => n + p.length, 0));
	let offset = 0;
	for (const p of pages) {
		out.set(p, offset);
		offset += p.length;
	}
	return out;
}

/**
 * Samples (at 48 kHz) an Opus packet decodes to, from its TOC byte (RFC 6716
 * §3.1), so granule positions do not depend on what an encoder reports as a
 * chunk's duration. `undefined` for a malformed packet.
 */
export function opusPacketSamples(packet: Uint8Array): number | undefined {
	if (packet.length === 0) return undefined;
	const toc = packet[0];
	const config = toc >> 3;
	// Frame length in units of 2.5 ms: SILK 10/20/40/60, hybrid 10/20, CELT 2.5/5/10/20 ms.
	const quarterMs =
		config < 12
			? [4, 8, 16, 24][config % 4]
			: config < 16
				? [4, 8][config % 2]
				: [1, 2, 4, 8][config % 4];
	let frames: number;
	switch (toc & 0x03) {
		case 0:
			frames = 1;
			break;
		case 1:
		case 2:
			frames = 2;
			break;
		default:
			if (packet.length < 2) return undefined;
			frames = packet[1] & 0x3f;
	}
	const samples = frames * quarterMs * 120; // 2.5 ms at 48 kHz = 120 samples
	// A packet holds at most 120 ms (RFC 6716 §3.2.5).
	return frames > 0 && samples <= 5760 ? samples : undefined;
}

/** Pre-skip from an OpusHead the encoder may hand out as its description. */
export function preSkipFrom(description: BufferSource | undefined): number {
	if (!description) return DEFAULT_PRE_SKIP;
	const bytes =
		description instanceof ArrayBuffer
			? new Uint8Array(description)
			: new Uint8Array(description.buffer, description.byteOffset, description.byteLength);
	const magic = new TextDecoder().decode(bytes.subarray(0, 8));
	if (bytes.length < 19 || magic !== 'OpusHead') return DEFAULT_PRE_SKIP;
	return bytes[10] | (bytes[11] << 8);
}

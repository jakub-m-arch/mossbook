// Voice notes (E-18, D-21): hands the microphone's samples to the page, which
// encodes them with WebCodecs. Loaded as an AudioWorklet module; a file of its
// own because the CSP allows scripts from the app only.
class VoiceCapture extends AudioWorkletProcessor {
	process(inputs) {
		const channel = inputs[0] && inputs[0][0];
		if (channel) this.port.postMessage(channel.slice());
		return true;
	}
}

registerProcessor('mossbook-voice-capture', VoiceCapture);

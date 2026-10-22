import { describe, expect, it } from 'vitest';

import {
	DEFAULT_PRE_SKIP,
	muxOggOpus,
	oggCrc,
	opusPacketSamples,
	preSkipFrom,
	type OpusPacket
} from './ogg';

const OPTIONS = { preSkip: 312, channels: 1, inputRate: 48_000, serial: 0x12345678 };

function packets(n: number): OpusPacket[] {
	return Array.from({ length: n }, (_, i) => ({
		data: Uint8Array.from([0xf8, i & 0xff, 0x55]),
		samples: 960
	}));
}

interface Page {
	flags: number;
	granule: bigint;
	sequence: number;
	serial: number;
	body: Uint8Array;
	crcOk: boolean;
}

/** Splits a file into pages, the way the core reads it. */
function pages(bytes: Uint8Array): Page[] {
	const out: Page[] = [];
	let offset = 0;
	while (offset < bytes.length) {
		const view = new DataView(bytes.buffer, bytes.byteOffset + offset);
		expect(new TextDecoder().decode(bytes.subarray(offset, offset + 4))).toBe('OggS');
		const segments = bytes[offset + 26];
		const lacing = bytes.subarray(offset + 27, offset + 27 + segments);
		const bodyLength = lacing.reduce((n, s) => n + s, 0);
		const length = 27 + segments + bodyLength;
		const raw = bytes.slice(offset, offset + length);
		const stored = view.getUint32(22, true);
		raw.fill(0, 22, 26);
		out.push({
			flags: bytes[offset + 5],
			granule: view.getBigUint64(6, true),
			serial: view.getUint32(14, true),
			sequence: view.getUint32(18, true),
			body: bytes.subarray(offset + 27 + segments, offset + length),
			crcOk: oggCrc(raw) === stored
		});
		offset += length;
	}
	return out;
}

describe('Ogg Opus muxer', () => {
	it('uses the RFC 3533 checksum', () => {
		// CRC-32/POSIX without the final xor: 0x765e7680 ^ 0xffffffff.
		expect(oggCrc(new TextEncoder().encode('123456789'))).toBe(0x89a1897f);
		expect(oggCrc(new Uint8Array())).toBe(0);
	});

	it('writes headers, then audio pages ending at the real length', () => {
		// 3 s of audio in 151 packets: the last one is mostly padding.
		const file = muxOggOpus(packets(151), { ...OPTIONS, totalSamples: 3 * 48_000 });
		const p = pages(file);
		expect(p.every((x) => x.crcOk)).toBe(true);
		expect(p.map((x) => x.sequence)).toEqual(p.map((_, i) => i));
		expect(new Set(p.map((x) => x.serial))).toEqual(new Set([0x12345678]));
		expect(new TextDecoder().decode(p[0].body.subarray(0, 8))).toBe('OpusHead');
		expect(p[0].flags).toBe(0x02);
		expect(p[0].body[9]).toBe(1);
		expect(p[0].body[10] | (p[0].body[11] << 8)).toBe(312);
		expect(new TextDecoder().decode(p[1].body.subarray(0, 8))).toBe('OpusTags');
		// One page per second of 20 ms packets.
		expect(p.length).toBe(2 + 4);
		expect(p[2].granule).toBe(BigInt(312 + 50 * 960));
		const last = p[p.length - 1];
		expect(last.flags).toBe(0x04);
		expect(last.granule).toBe(BigInt(312 + 3 * 48_000));
	});

	it('keeps no metadata but a vendor string', () => {
		const file = muxOggOpus(packets(1), { ...OPTIONS, totalSamples: 960 });
		const tags = new TextDecoder().decode(pages(file)[1].body);
		expect(tags).toContain('Mossbook');
		expect(tags.length).toBe(8 + 4 + 'Mossbook'.length + 4);
	});

	it('refuses an empty recording', () => {
		expect(() => muxOggOpus([], { ...OPTIONS, totalSamples: 0 })).toThrow();
	});

	it('reads the pre-skip from an OpusHead description', () => {
		const head = new Uint8Array(19);
		head.set(new TextEncoder().encode('OpusHead'));
		head[10] = 0x38;
		head[11] = 0x01;
		expect(preSkipFrom(head)).toBe(312);
		head[10] = 0x00;
		head[11] = 0x02;
		expect(preSkipFrom(head.buffer)).toBe(512);
		expect(preSkipFrom(undefined)).toBe(DEFAULT_PRE_SKIP);
		expect(preSkipFrom(new Uint8Array(4))).toBe(DEFAULT_PRE_SKIP);
	});

	it('counts samples from the Opus TOC byte', () => {
		const toc = (config: number, code: number) => (config << 3) | code;
		// CELT 20 ms, one frame: what WebCodecs produces at 32 kbit/s.
		expect(opusPacketSamples(Uint8Array.of(toc(31, 0), 1, 2))).toBe(960);
		// CELT 2.5 ms; SILK 60 ms; hybrid 10 ms.
		expect(opusPacketSamples(Uint8Array.of(toc(16, 0)))).toBe(120);
		expect(opusPacketSamples(Uint8Array.of(toc(3, 0)))).toBe(2880);
		expect(opusPacketSamples(Uint8Array.of(toc(12, 0)))).toBe(480);
		// Two frames (codes 1 and 2), or a counted number (code 3).
		expect(opusPacketSamples(Uint8Array.of(toc(31, 1)))).toBe(1920);
		expect(opusPacketSamples(Uint8Array.of(toc(31, 2)))).toBe(1920);
		expect(opusPacketSamples(Uint8Array.of(toc(31, 3), 3))).toBe(2880);
		// Malformed: empty, no frame count, zero frames, over 120 ms.
		expect(opusPacketSamples(new Uint8Array())).toBeUndefined();
		expect(opusPacketSamples(Uint8Array.of(toc(31, 3)))).toBeUndefined();
		expect(opusPacketSamples(Uint8Array.of(toc(31, 3), 0))).toBeUndefined();
		expect(opusPacketSamples(Uint8Array.of(toc(3, 3), 3))).toBeUndefined();
	});
});

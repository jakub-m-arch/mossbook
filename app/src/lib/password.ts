// Local password strength estimate for the setup meter (V-1). Deliberately
// simple and dependency-free: it guides the user, it does not gate anything
// beyond the 8-character minimum enforced in Rust.

export const MIN_PASSWORD_CHARS = 8;

/** 0 = too short, 1 = weak, 2 = okay, 3 = strong. */
export type Strength = 0 | 1 | 2 | 3;

const COMMON = [
	'password',
	'haslo',
	'qwerty',
	'zaq12wsx',
	'123456',
	'abc123',
	'iloveyou',
	'mossbook',
	'admin',
	'letmein',
	'пароль'
];

export function passwordStrength(password: string): Strength {
	const chars = [...password];
	if (chars.length < MIN_PASSWORD_CHARS) return 0;

	const lower = password.toLowerCase();
	if (COMMON.some((c) => lower.includes(c)) || new Set(chars).size <= 3 || isSequence(lower)) {
		return 1;
	}

	const classes = [/\p{Ll}/u, /\p{Lu}/u, /\p{N}/u, /[^\p{L}\p{N}]/u].filter((re) =>
		re.test(password)
	).length;
	const words = password.trim().split(/\s+/).length;

	// Long passphrases are strong even in lower case.
	if (chars.length >= 20 || (chars.length >= 16 && words >= 3)) return 3;
	if (chars.length >= 12 && classes >= 3) return 3;
	if (chars.length >= 12 || classes >= 3) return 2;
	return 1;
}

/** `abcdefgh`, `12345678`, `87654321` and similar runs. */
function isSequence(text: string): boolean {
	const codes = [...text].map((c) => c.codePointAt(0) ?? 0);
	const steps = new Set(codes.slice(1).map((c, i) => c - codes[i]));
	return steps.size === 1 && [...steps].every((s) => Math.abs(s) === 1);
}

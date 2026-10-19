// In-browser stand-in for the Tauri backend, injected before the app loads.
// It mirrors the command contract of app/src-tauri (codes, state machine) so the
// UI can be exercised in a plain browser. Synthetic data only.
(() => {
	const PASSWORD = 'correct horse battery';
	const WORDS = Array.from({ length: 24 }, (_, i) => `word${i + 1}`);
	const POSITIONS = [2, 10, 20];

	const state = {
		vault: window.localStorage.getItem('fake-vault') ?? 'no_vault',
		failures: 0,
		blockedUntil: 0,
		entries: []
	};
	const save = () => window.localStorage.setItem('fake-vault', state.vault);
	const fail = (code, extra = {}) => Promise.reject({ code, message: code, ...extra });

	const callbacks = new Map();
	const listeners = new Map();
	let nextId = 1;

	function emit(event, payload) {
		for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
	}

	function unlockAttempt(ok) {
		const now = Date.now();
		if (now < state.blockedUntil)
			return fail('throttled', { retry_after_ms: state.blockedUntil - now });
		if (!ok) {
			state.failures += 1;
			const wait = Math.min(1000 * 2 ** (state.failures - 1), 60000);
			state.blockedUntil = now + wait;
			return fail('wrong_password', { retry_after_ms: wait });
		}
		state.failures = 0;
		state.vault = 'unlocked';
		save();
		return Promise.resolve(null);
	}

	const commands = {
		clock_sync: () => null,
		activity_ping: () => null,
		vault_status: () => ({
			state: state.vault,
			unreadable_records: 0,
			...(Date.now() < state.blockedUntil
				? { retry_after_ms: state.blockedUntil - Date.now() }
				: {})
		}),
		vault_create: ({ password }) => {
			if ([...password].length < 8) return fail('password_too_short');
			state.vault = 'creating';
			return { words: WORDS, positions: POSITIONS };
		},
		vault_cancel_create: () => {
			state.vault = 'no_vault';
			return null;
		},
		vault_confirm_recovery: ({ answers }) => {
			const ok =
				answers.length === 3 &&
				answers.every((a) => POSITIONS.includes(a.position) && WORDS[a.position] === a.word.trim());
			if (!ok) return fail('recovery_mismatch');
			state.vault = 'unlocked';
			save();
			return null;
		},
		vault_unlock: ({ password }) => unlockAttempt(password === PASSWORD),
		vault_unlock_recovery: ({ phrase }) =>
			phrase.trim().split(/\s+/).join(' ') === WORDS.join(' ')
				? unlockAttempt(true)
				: fail('wrong_recovery_key'),
		vault_lock: () => {
			if (state.vault === 'unlocked') {
				state.vault = 'locked';
				save();
				emit('vault-locked', 'manual');
			}
			return null;
		},
		entries_list: () => (state.vault === 'unlocked' ? state.entries : fail('locked')),
		'plugin:event|listen': ({ event, handler }) => {
			listeners.set(event, [...(listeners.get(event) ?? []), handler]);
			return handler;
		},
		'plugin:event|unlisten': () => null
	};

	window.__TAURI_INTERNALS__ = {
		metadata: {
			currentWindow: { label: 'main' },
			currentWebview: { windowLabel: 'main', label: 'main' }
		},
		transformCallback(callback, once = false) {
			const id = nextId++;
			callbacks.set(id, (data) => {
				if (once) callbacks.delete(id);
				return callback?.(data);
			});
			return id;
		},
		unregisterCallback: (id) => callbacks.delete(id),
		invoke(cmd, args = {}) {
			const handler = commands[cmd];
			if (!handler) return fail('internal', { message: `fake backend: no ${cmd}` });
			return Promise.resolve().then(() => handler(args));
		},
		convertFileSrc: (path) => path
	};
	window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

	/** Test hooks: simulate Rust-side events (auto-lock, focus loss). */
	window.__fake = {
		emit,
		lockFromRust(reason) {
			state.vault = 'locked';
			save();
			emit('vault-locked', reason);
		}
	};
})();

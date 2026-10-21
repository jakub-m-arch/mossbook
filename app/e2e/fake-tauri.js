// In-browser stand-in for the Tauri backend, injected before the app loads.
// It mirrors the command contract of app/src-tauri (codes, state machine) so the
// UI can be exercised in a plain browser. Synthetic data only.
(() => {
	let password = 'correct horse battery';
	const BACKUP_PASSWORD = 'backup password';
	const WORDS = Array.from({ length: 24 }, (_, i) => `word${i + 1}`);
	const POSITIONS = [2, 10, 20];

	const kindTag = (kind) => (typeof kind === 'string' ? kind : Object.keys(kind)[0]);
	const texts = (kind) =>
		typeof kind === 'string'
			? []
			: Object.values(Object.values(kind)[0]).filter((v) => typeof v === 'string');
	const tagName = (id) => state.tags.find((t) => t.id === id)?.name;
	const emptyKind = (tag) =>
		({
			mood: { mood: { energy: null, emotions: [] } },
			event: { event: { what: '', place: '', body: '', thoughts: '', helped: '' } },
			fear: {
				fear: { trigger: '', symptoms: [], avoidance: null, strategy: null, intensity_after: null }
			},
			sleep: {
				sleep: { bedtime: null, wake: null, quality: null, awakenings: null, nightmare: false }
			},
			nightmare: { nightmare: { content: '', rescripted: '', rehearsals: [] } },
			note: 'note'
		})[tag];

	const state = {
		vault: window.localStorage.getItem('fake-vault') ?? 'no_vault',
		failures: 0,
		blockedUntil: 0,
		entries: [],
		tags: [],
		settings: {
			autolock: 'immediately',
			locale: null,
			crisis_country: 'PL',
			reminders: [],
			auto_backup: true,
			last_export: null
		},
		autoLatest: null,
		pendingBackup: false,
		inbox: [],
		plan: {
			warning_signs: [],
			self_coping: [],
			calming_people_places: [],
			contacts: [],
			professionals: [],
			safe_environment: []
		}
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
		// Move quick entries written while locked into the journal (E-15).
		for (const { kind, value } of state.inbox.splice(0)) {
			commands.quick_entry({ kind, value });
		}
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
		vault_unlock: (args) => unlockAttempt(args.password === password),
		vault_unlock_recovery: ({ phrase }) =>
			phrase.trim().split(/\s+/).join(' ') === WORDS.join(' ')
				? unlockAttempt(true)
				: fail('wrong_recovery_key'),
		vault_lock: () => {
			if (state.vault === 'unlocked') {
				state.vault = 'locked';
				if (state.settings.auto_backup) state.autoLatest = new Date().toISOString();
				save();
				emit('vault-locked', 'manual');
			}
			return null;
		},
		entries_list: ({ filter = {} }) => {
			if (state.vault !== 'unlocked') return fail('locked');
			const fold = (t) =>
				t.normalize('NFKD').replace(/\p{M}/gu, '').replace(/ł/gi, 'l').toLowerCase();
			const terms = fold(filter.text ?? '')
				.split(/\s+/)
				.filter(Boolean);
			return state.entries
				.filter((e) => !filter.kinds?.length || filter.kinds.includes(kindTag(e.kind)))
				.filter((e) => !filter.tags?.length || e.tags.some((t) => filter.tags.includes(t)))
				.filter((e) => !filter.incomplete_only || e.incomplete)
				.filter((e) => !filter.from || Date.parse(e.occurred_at) >= Date.parse(filter.from))
				.filter((e) => !filter.to || Date.parse(e.occurred_at) < Date.parse(filter.to))
				.filter((e) => {
					const hay = fold(
						[e.note, ...texts(e.kind), ...e.tags.map((id) => tagName(id))].join(' ')
					);
					return terms.every((term) => hay.includes(term));
				})
				.sort((a, b) => Date.parse(b.occurred_at) - Date.parse(a.occurred_at));
		},
		entry_get: ({ id }) => state.entries.find((e) => e.id === id) ?? fail('not_found'),
		entry_save: ({ draft }) => {
			const existing = draft.id && state.entries.find((e) => e.id === draft.id);
			if (draft.id && !existing) return fail('not_found');
			const now = new Date().toISOString();
			const entry = {
				id: existing?.id ?? `entry-${nextId++}`,
				schema: 1,
				created_at: existing?.created_at ?? now,
				updated_at: now,
				occurred_at: draft.occurred_at ?? existing?.occurred_at ?? now,
				kind: draft.kind,
				mood: draft.mood ?? null,
				intensity: draft.intensity ?? null,
				tags: [...new Set(draft.tags ?? [])],
				note: draft.note ?? '',
				incomplete: draft.incomplete ?? false,
				sensitive: draft.sensitive ?? false
			};
			if (
				(entry.mood !== null && (entry.mood < 1 || entry.mood > 5)) ||
				entry.tags.some((t) => !tagName(t))
			)
				return fail('invalid_field');
			state.entries = [...state.entries.filter((e) => e.id !== entry.id), entry];
			return entry;
		},
		entry_delete: ({ id }) => {
			const before = state.entries.length;
			state.entries = state.entries.filter((e) => e.id !== id);
			return before !== state.entries.length;
		},
		quick_entry: ({ kind, value }) =>
			commands.entry_save({
				draft: {
					kind: emptyKind(kind),
					mood: kind === 'mood' ? value : null,
					intensity: kind === 'mood' ? null : value,
					incomplete: true,
					sensitive: kind === 'nightmare'
				}
			}),
		quick_entry_locked: ({ kind, value }) => {
			if (state.vault !== 'locked') return fail('invalid_state');
			state.inbox.push({ kind, value });
			return null;
		},
		tags_list: () => [...state.tags].sort((a, b) => a.name.localeCompare(b.name)),
		tag_create: ({ name, color }) =>
			commands.tag_save({ tag: { id: `tag-${nextId++}`, name, color }, create: true }),
		tag_save: ({ tag, create }) => {
			const name = tag.name.trim();
			if (!name) return fail('invalid_field');
			if (state.tags.some((t) => t.id !== tag.id && t.name.toLowerCase() === name.toLowerCase()))
				return fail('duplicate_tag_name');
			if (!create && !state.tags.some((t) => t.id === tag.id)) return fail('not_found');
			const saved = { ...tag, name };
			state.tags = [...state.tags.filter((t) => t.id !== tag.id), saved];
			return saved;
		},
		tag_delete: ({ id }) => {
			state.entries = state.entries.map((e) => ({ ...e, tags: e.tags.filter((t) => t !== id) }));
			const before = state.tags.length;
			state.tags = state.tags.filter((t) => t.id !== id);
			return before !== state.tags.length;
		},
		tag_merge: ({ from, into }) => {
			state.entries = state.entries.map((e) =>
				e.tags.includes(from)
					? { ...e, tags: [...new Set(e.tags.map((t) => (t === from ? into : t)))] }
					: e
			);
			state.tags = state.tags.filter((t) => t.id !== from);
			return null;
		},
		clipboard_clear_if: () => false,
		app_info: () => ({
			version: '0.1.0',
			built_at: Math.floor(Date.now() / 1000) - 120 * 86400,
			releases_url: 'https://github.com/jakub-m-arch/mossbook/releases'
		}),
		open_releases: () => {
			window.__fake.calls.push('open_releases');
			return null;
		},
		// Simplified mirror of core::query::patterns (tags, time of day, strategies).
		stats_patterns: ({ range }) => {
			if (state.vault !== 'unlocked') return fail('locked');
			const days = { week: 7, month: 30, three_months: 90 }[range];
			const list = state.entries.filter(
				(e) => Date.parse(e.occurred_at) >= Date.now() - days * 86400000
			);
			const out = {
				entries: list.length,
				enough_data: list.length >= 14,
				top_tags: [],
				top_triggers: [],
				mood_by_sleep: [],
				mood_by_time_of_day: [],
				mood_by_weekday: [],
				strategies: []
			};
			if (!out.enough_data) return out;
			const slots = ['night', 'morning', 'afternoon', 'evening'];
			const tod = new Map();
			const strategies = new Map();
			for (const e of list) {
				const h = new Date(e.occurred_at).getHours();
				const key = slots[h < 5 ? 0 : h < 12 ? 1 : h < 18 ? 2 : 3];
				if (e.mood !== null) {
					const [sum, n] = tod.get(key) ?? [0, 0];
					tod.set(key, [sum + e.mood, n + 1]);
				}
				const f = typeof e.kind === 'object' && e.kind.fear;
				if (f && f.strategy && f.intensity_after !== null && e.intensity !== null) {
					const [drop, n] = strategies.get(f.strategy) ?? [0, 0];
					strategies.set(f.strategy, [drop + e.intensity - f.intensity_after, n + 1]);
				}
			}
			out.mood_by_time_of_day = slots
				.filter((k) => tod.has(k))
				.map((k) => ({ key: k, average: tod.get(k)[0] / tod.get(k)[1], count: tod.get(k)[1] }));
			out.strategies = [...strategies]
				.filter(([, [, n]]) => n >= 2)
				.map(([label, [drop, n]]) => ({ label, average_drop: drop / n, count: n }));
			return out;
		},
		report_pdf: ({ report, password }) => {
			if (state.vault !== 'unlocked') return fail('locked');
			window.__fake.reports.push(report);
			window.__fake.reportPasswords.push(password ?? null);
			return true;
		},
		stats_mood: ({ range }) => {
			if (state.vault !== 'unlocked') return fail('locked');
			const days = { week: 7, month: 30, three_months: 90 }[range];
			const from = Date.now() - days * 86400000;
			const byDay = new Map();
			for (const e of state.entries) {
				const t = Date.parse(e.occurred_at);
				if (e.mood === null || t < from) continue;
				const d = new Date(t);
				const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
				const [sum, n] = byDay.get(key) ?? [0, 0];
				byDay.set(key, [sum + e.mood, n + 1]);
			}
			return [...byDay.entries()]
				.sort(([a], [b]) => a.localeCompare(b))
				.map(([date, [sum, count]]) => ({ date, average: sum / count, count }));
		},
		settings_get: () => (state.vault === 'unlocked' ? state.settings : fail('locked')),
		settings_save: ({ settings }) => {
			if (state.vault !== 'unlocked') return fail('locked');
			// Like Rust: last_export is set only by backup_export.
			state.settings = { ...settings, last_export: state.settings.last_export };
			return state.settings;
		},
		vault_verify_recovery: ({ phrase }) => {
			if (state.vault !== 'unlocked') return fail('locked');
			const words = phrase.trim().split(/\s+/);
			if (words.length !== 24) return fail('invalid_recovery_phrase');
			return words.join(' ') === WORDS.join(' ') ? null : fail('wrong_recovery_key');
		},
		backup_info: () =>
			state.vault === 'unlocked'
				? {
						auto_enabled: state.settings.auto_backup,
						auto_latest: state.autoLatest,
						auto_count: state.autoLatest ? 1 : 0,
						auto_failed: false,
						can_reveal: true,
						last_export: state.settings.last_export
					}
				: fail('locked'),
		backup_reveal: () => (state.autoLatest ? null : fail('not_found')),
		export_json: () => {
			if (state.vault !== 'unlocked') return fail('locked');
			window.__fake.jsonExports += 1;
			return true;
		},
		vault_change_password: ({ old, new: next }) => {
			if (old !== password) return fail('wrong_password');
			if ([...next].length < 8) return fail('password_too_short');
			password = next;
			return null;
		},
		vault_destroy: (args) => {
			if (args.password !== password) return fail('wrong_password');
			state.vault = 'no_vault';
			state.entries = [];
			save();
			return null;
		},
		backup_export: () => {
			if (state.vault !== 'unlocked') return fail('locked');
			state.settings = { ...state.settings, last_export: new Date().toISOString() };
			return true;
		},
		// The fake backup holds one synthetic note and opens with BACKUP_PASSWORD.
		backup_pick: () => {
			state.pendingBackup = true;
			return { records: 3, same_vault: false };
		},
		backup_import: ({ secret, kind, mode }) => {
			if (!state.pendingBackup) return fail('invalid_state');
			if (state.vault === 'locked') return fail('locked');
			const ok =
				kind === 'password' ? secret === BACKUP_PASSWORD : secret.trim().split(/\s+/).length === 24;
			if (!ok) return fail(kind === 'password' ? 'wrong_password' : 'wrong_recovery_key');
			state.pendingBackup = false;
			const note = {
				id: 'backup-entry',
				schema: 1,
				created_at: '2026-10-01T10:00:00+02:00',
				updated_at: '2026-10-01T10:00:00+02:00',
				occurred_at: '2026-10-01T10:00:00+02:00',
				kind: 'note',
				mood: null,
				intensity: null,
				tags: [],
				note: 'Wpis z kopii',
				incomplete: false,
				sensitive: false
			};
			const restored = state.vault !== 'unlocked';
			if (restored) {
				password = BACKUP_PASSWORD;
				state.vault = 'unlocked';
				save();
			}
			const replaced = mode === 'replace' || restored;
			state.entries = replaced ? [note] : [...state.entries.filter((e) => e.id !== note.id), note];
			return { restored, report: { added: 1, updated: 0, unchanged: 0, skipped: 0 } };
		},
		plan_get: () => (state.vault === 'unlocked' ? state.plan : fail('locked')),
		plan_save: ({ plan }) => {
			if (state.vault !== 'unlocked') return fail('locked');
			const clean = (list) => list.filter((item) => item.trim() !== '');
			const people = (list) => list.filter((c) => c.name.trim() || c.phone.trim());
			state.plan = {
				warning_signs: clean(plan.warning_signs),
				self_coping: clean(plan.self_coping),
				calming_people_places: clean(plan.calming_people_places),
				contacts: people(plan.contacts),
				professionals: people(plan.professionals),
				safe_environment: clean(plan.safe_environment)
			};
			return state.plan;
		},
		crisis_lines: ({ country }) => {
			const data = window.__crisisData;
			return {
				countries: [data.country],
				directory: country.toUpperCase() === data.country ? data : null
			};
		},
		dial: ({ number }) => {
			const digits = (n) => (n.trim().startsWith('+') ? '+' : '') + n.replace(/\D/g, '');
			const wanted = digits(number);
			const known = window.__crisisData.lines.some((l) => digits(l.number) === wanted);
			const contact =
				state.vault === 'unlocked' &&
				[...state.plan.contacts, ...state.plan.professionals].some(
					(c) => digits(c.phone) === wanted
				);
			if (!wanted || !(known || contact)) return fail('invalid_field');
			window.__fake.calls.push(wanted);
			return null;
		},
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
		/** Adds synthetic entries directly (for tests that need many of them). */
		seed(entries) {
			state.entries.push(...entries);
		},
		calls: [],
		reports: [],
		reportPasswords: [],
		jsonExports: 0,
		emit,
		lockFromRust(reason) {
			state.vault = 'locked';
			save();
			emit('vault-locked', reason);
		}
	};
})();

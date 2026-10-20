import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn<(command: string, args?: unknown) => Promise<unknown>>(() =>
	Promise.resolve(undefined)
);
vi.mock('@tauri-apps/api/core', () => ({
	invoke: (command: string, args?: unknown) => invoke(command, args)
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

const api = await import('./index');

/** Command names registered in Rust (app/src-tauri/src/command_names.rs). */
function rustCommands(): string[] {
	const source = readFileSync(
		new URL('../../../src-tauri/src/command_names.rs', import.meta.url),
		'utf8'
	);
	return [...source.matchAll(/^\s+"([a-z_]+)",$/gm)].map((m) => m[1]);
}

/** Calls every wrapper with dummy arguments and records the invoked command names. */
async function invokedCommands(): Promise<string[]> {
	const calls: Array<() => Promise<unknown>> = [
		api.clockSync,
		api.activityPing,
		api.vaultStatus,
		() => api.vaultCreate('pw'),
		api.vaultCancelCreate,
		() => api.vaultConfirmRecovery([]),
		() => api.vaultUnlock('pw'),
		() => api.vaultUnlockRecovery('phrase', 'pw'),
		api.vaultLock,
		() => api.vaultChangePassword('a', 'b'),
		() => api.vaultDestroy('pw'),
		() => api.entriesList(),
		() => api.entryGet('id'),
		() => api.entrySave({ kind: 'note' }),
		() => api.entryDelete('id'),
		() => api.quickEntry('fear', 5),
		() => api.quickEntryLocked('fear', 5),
		api.tagsList,
		() => api.tagCreate('n', '#000000'),
		() => api.tagSave({ id: 'id', name: 'n', color: '#000000' }),
		() => api.tagDelete('id'),
		() => api.tagMerge('a', 'b'),
		api.planGet,
		() =>
			api.planSave({
				warning_signs: [],
				self_coping: [],
				calming_people_places: [],
				contacts: [],
				professionals: [],
				safe_environment: []
			}),
		api.settingsGet,
		() =>
			api.settingsSave({
				autolock: 'immediately',
				locale: 'pl',
				crisis_country: 'PL',
				reminders: []
			}),
		() => api.statsMood('week'),
		() => api.statsPatterns('week'),
		() => api.clipboardClearIf('copied'),
		() => api.crisisLines('PL'),
		() => api.dial('112'),
		api.backupExport,
		api.backupPick,
		() => api.backupImport('pw', 'password', 'merge'),
		() =>
			api.reportPdf({
				title: 'r',
				subtitle: '',
				summary: [],
				sections: [],
				footer: '',
				page_label: ''
			})
	];
	for (const call of calls) await call();
	return invoke.mock.calls.map((c) => c[0]);
}

describe('api wrappers', () => {
	beforeEach(() => invoke.mockClear());

	it('cover exactly the commands registered in Rust, in order', async () => {
		expect(await invokedCommands()).toEqual(rustCommands());
	});

	it('pass camelCase arguments that Tauri maps to snake_case parameters', async () => {
		await api.vaultUnlockRecovery('phrase', 'new pw');
		expect(invoke).toHaveBeenLastCalledWith('vault_unlock_recovery', {
			phrase: 'phrase',
			newPassword: 'new pw'
		});
		await api.vaultChangePassword('old pw', 'new pw');
		expect(invoke).toHaveBeenLastCalledWith('vault_change_password', {
			old: 'old pw',
			new: 'new pw'
		});
		await api.clockSync();
		const args = invoke.mock.lastCall?.[1] as { offsetMinutes: number };
		expect(args.offsetMinutes).toBe(-new Date().getTimezoneOffset());
	});

	it('recognizes AppError rejections', () => {
		expect(api.isAppError({ code: 'locked', message: 'the vault is locked' })).toBe(true);
		expect(api.isAppError(new Error('x'))).toBe(false);
		expect(api.isAppError(null)).toBe(false);
	});
});

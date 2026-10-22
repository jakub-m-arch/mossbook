// App-wide session wiring: clock sync, auto-lock events, the privacy curtain
// and activity pings for the Rust idle timer (SPEC §8.3).

import { goto } from '$app/navigation';
import { resolve } from '$app/paths';

import {
	activityPing,
	clipboardClearIf,
	clockSync,
	isAppError,
	onPrivacyCurtain,
	onVaultLocked,
	syncNow
} from './api/index.js';
import { t } from './i18n/index.svelte.js';

/** Dispatched on `window` when a sync brought changes; screens reload their data. */
export const JOURNAL_CHANGED = 'mossbook:journal-changed';

/** Minimum gap between activity pings. */
const PING_INTERVAL_MS = 20_000;

/** Copied text is cleared from the clipboard after this long (SPEC §2). */
export const CLIPBOARD_CLEAR_MS = 60_000;

/** The text a `copy` event puts on the clipboard, including from inputs. */
export function copiedText(target: EventTarget | null): string {
	if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
		const { selectionStart: start, selectionEnd: end, value } = target;
		if (start !== null && end !== null && end > start) return value.slice(start, end);
	}
	return window.getSelection()?.toString() ?? '';
}

/**
 * Writes `text` to the clipboard and schedules the same 60 s clear as a user
 * copy. `navigator.clipboard.writeText` does not fire a `copy` event, so the
 * document listener above would not see it.
 */
export async function copyWithClear(text: string): Promise<void> {
	if (!navigator.clipboard) throw new Error('clipboard unavailable');
	await navigator.clipboard.writeText(text);
	setTimeout(() => clipboardClearIf(text).catch(() => {}), CLIPBOARD_CLEAR_MS);
}

let covered = $state(false);

/** Whether the privacy curtain covers the content (desktop focus loss, D-11). */
export function curtainCovered(): boolean {
	return covered;
}

/** Sends the user to the lock screen. */
export function toLockScreen(): Promise<void> {
	return goto(resolve('/lock'), { replace: true });
}

/** Translated, user-facing text for a failed command. */
export function errorText(e: unknown): string {
	if (isAppError(e)) {
		const key = `errors.${e.code}`;
		const text = t(key);
		return text === key ? t('errors.generic') : text;
	}
	return t('errors.generic');
}

/** Goes to the lock screen if a command failed because the vault is locked. */
export async function handleLocked(e: unknown): Promise<boolean> {
	if (isAppError(e) && e.code === 'locked') {
		await toLockScreen();
		return true;
	}
	return false;
}

/** Installs listeners once at startup; returns a cleanup function. */
export function installSessionHooks(): () => void {
	const cleanups: Array<() => void> = [];
	let lastPing = 0;

	const ping = () => {
		const now = Date.now();
		if (now - lastPing >= PING_INTERVAL_MS) {
			lastPing = now;
			activityPing().catch(() => {});
		}
	};
	const onVisible = () => {
		if (document.visibilityState !== 'visible') return;
		clockSync().catch(() => {});
		// Changes another device made meanwhile (D-22); ignored while locked.
		syncNow()
			.then((status) => {
				if (status.received > 0) window.dispatchEvent(new Event(JOURNAL_CHANGED));
			})
			.catch(() => {});
	};

	const onCopy = (event: ClipboardEvent) => {
		const text = copiedText(event.target);
		if (!text.trim()) return;
		const timer = setTimeout(() => clipboardClearIf(text).catch(() => {}), CLIPBOARD_CLEAR_MS);
		cleanups.push(() => clearTimeout(timer));
	};
	document.addEventListener('copy', onCopy);
	cleanups.push(() => document.removeEventListener('copy', onCopy));

	clockSync().catch(() => {});
	for (const type of ['pointerdown', 'keydown', 'wheel', 'touchstart'] as const) {
		window.addEventListener(type, ping, { passive: true });
		cleanups.push(() => window.removeEventListener(type, ping));
	}
	document.addEventListener('visibilitychange', onVisible);
	cleanups.push(() => document.removeEventListener('visibilitychange', onVisible));

	onVaultLocked(() => {
		covered = false;
		void toLockScreen();
	}).then((unlisten) => cleanups.push(unlisten));
	onPrivacyCurtain((value) => (covered = value)).then((unlisten) => cleanups.push(unlisten));

	return () => cleanups.forEach((cleanup) => cleanup());
}

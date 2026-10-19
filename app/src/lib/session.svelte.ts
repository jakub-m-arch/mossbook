// App-wide session wiring: clock sync, auto-lock events, the privacy curtain
// and activity pings for the Rust idle timer (SPEC §8.3).

import { goto } from '$app/navigation';
import { resolve } from '$app/paths';

import { activityPing, clockSync, isAppError, onPrivacyCurtain, onVaultLocked } from './api';
import { t } from './i18n/index.svelte';

/** Minimum gap between activity pings. */
const PING_INTERVAL_MS = 20_000;

let covered = $state(false);

/** Whether the privacy curtain covers the content (desktop focus loss, D-11). */
export function curtainCovered(): boolean {
	return covered;
}

/** Sends the user to the lock screen. */
export function toLockScreen(): Promise<void> {
	return goto(resolve('/lock'), { replaceState: true });
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
		if (document.visibilityState === 'visible') clockSync().catch(() => {});
	};

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

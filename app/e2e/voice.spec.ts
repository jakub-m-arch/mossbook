import type { Page } from '@playwright/test';

import { PASSWORD, createVault, expect, test } from './helpers';

/** Test hooks exposed by e2e/fake-tauri.js. */
type FakeWindow = Window & {
	__fake: {
		voiceSaves: { duration_ms: number }[];
		voiceCount(): number;
		readonly screenOn: boolean;
		goBackground(): void;
	};
};

const screenOn = (page: Page) =>
	page.evaluate(() => (window as unknown as FakeWindow).__fake.screenOn);

/** The phone turned the screen off: the page is hidden and Rust sees the background. */
async function screenTurnsOff(page: Page) {
	await page.evaluate(() => {
		(window as unknown as FakeWindow).__fake.goBackground();
		Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
		document.dispatchEvent(new Event('visibilitychange'));
	});
}

async function screenTurnsOn(page: Page) {
	await page.evaluate(() => {
		Object.defineProperty(document, 'visibilityState', {
			configurable: true,
			get: () => 'visible'
		});
		document.dispatchEvent(new Event('visibilitychange'));
	});
}

test.beforeEach(async ({ page }) => {
	await createVault(page);
});

test('voice note: record, save with the entry, play, remove', async ({ page }) => {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	await expect(page.getByRole('heading', { name: 'Notatka' })).toBeVisible();

	const voice = page.getByRole('region', { name: 'Notatka głosowa' });
	await expect(voice.getByText('Do 5 minut.')).toBeVisible();
	await voice.getByRole('button', { name: 'Nagraj notatkę głosową' }).click();
	await expect(voice.getByRole('status')).toContainText('Nagrywanie');
	await page.waitForTimeout(2200);
	await voice.getByRole('button', { name: 'Zakończ nagrywanie' }).click();

	// Encoded in the page (WebCodecs Opus in Ogg) and stored; the length is the file's.
	const play = voice.getByRole('button', { name: 'Odtwórz notatkę głosową' });
	await expect(play).toBeVisible();
	const saved = await page.evaluate(() => (window as unknown as FakeWindow).__fake.voiceSaves);
	expect(saved).toHaveLength(1);
	expect(saved[0].duration_ms).toBeGreaterThan(1500);
	expect(saved[0].duration_ms).toBeLessThan(4000);
	await expect(voice).toContainText(/(?<![\d:])0:0[1-3]/);

	await page.getByLabel('Notatka', { exact: true }).fill('Synthetic note with a recording');
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
	const card = page.getByRole('listitem').filter({ hasText: 'Synthetic note' });
	await expect(card).toContainText(/(?<![\d:])0:0[1-3]/);

	// Played back from memory: the audio element advances.
	await card.getByRole('link').click();
	await play.click();
	await expect(voice.getByRole('button', { name: 'Wstrzymaj' })).toBeVisible();
	await expect
		.poll(() => page.locator('audio').evaluate((a: HTMLAudioElement) => a.currentTime))
		.toBeGreaterThan(0.2);
	// Two seconds of audio play to the end.
	await expect(play).toBeVisible({ timeout: 5000 });

	// Removed in the form and saved: gone from the entry and from storage.
	await voice.getByRole('button', { name: 'Usuń nagranie' }).click();
	await expect(voice.getByText('Nagranie zniknie z wpisu po zapisaniu.')).toBeVisible();
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
	await expect(card).toBeVisible();
	// A recording's length (0:0x), not the time of day: 10:02 must not count.
	await expect(card).not.toContainText(/(?<![\d:])0:0\d/);
	expect(await page.evaluate(() => (window as unknown as FakeWindow).__fake.voiceCount())).toBe(0);
});

test('voice note: a recording in progress is dropped when the journal locks', async ({ page }) => {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	const voice = page.getByRole('region', { name: 'Notatka głosowa' });
	await voice.getByRole('button', { name: 'Nagraj notatkę głosową' }).click();
	await expect(voice.getByRole('status')).toContainText('Nagrywanie');
	await page.evaluate(() =>
		(window as unknown as { __fake: { lockFromRust(r: string): void } }).__fake.lockFromRust(
			'background'
		)
	);
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
	expect(await page.evaluate(() => (window as unknown as FakeWindow).__fake.voiceCount())).toBe(0);
	// The microphone is released.
	await page.waitForTimeout(300);
	const live = await page.evaluate(() =>
		navigator.mediaDevices.enumerateDevices().then(() => document.querySelectorAll('audio').length)
	);
	expect(live).toBe(0);
});

test('voice note: the screen stays on only while recording', async ({ page }) => {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	const voice = page.getByRole('region', { name: 'Notatka głosowa' });
	expect(await screenOn(page)).toBe(false);
	await voice.getByRole('button', { name: 'Nagraj notatkę głosową' }).click();
	await expect(voice.getByRole('status')).toContainText('Nagrywanie');
	await expect.poll(() => screenOn(page)).toBe(true);
	await voice.getByRole('button', { name: 'Przerwij' }).click();
	await expect.poll(() => screenOn(page)).toBe(false);

	await voice.getByRole('button', { name: 'Nagraj notatkę głosową' }).click();
	await expect.poll(() => screenOn(page)).toBe(true);
	await page.waitForTimeout(1200);
	await voice.getByRole('button', { name: 'Zakończ nagrywanie' }).click();
	await expect(voice.getByRole('button', { name: 'Odtwórz notatkę głosową' })).toBeVisible();
	await expect.poll(() => screenOn(page)).toBe(false);
});

test('voice note: what was said is kept when the screen turns off mid-recording', async ({
	page
}) => {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	const voice = page.getByRole('region', { name: 'Notatka głosowa' });
	await voice.getByRole('button', { name: 'Nagraj notatkę głosową' }).click();
	await expect(voice.getByRole('status')).toContainText('Nagrywanie');
	await page.waitForTimeout(1200);

	await screenTurnsOff(page);
	// The lock waited for the recording to be stored and the form kept.
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
	expect(await page.evaluate(() => (window as unknown as FakeWindow).__fake.voiceCount())).toBe(1);
	expect(await screenOn(page)).toBe(false);

	await screenTurnsOn(page);
	await page.getByLabel('Hasło', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Otwórz' }).click();
	await page.getByRole('button', { name: 'Wróć do wpisu' }).click();
	await expect(voice.getByRole('button', { name: 'Odtwórz notatkę głosową' })).toBeVisible();
	await expect(voice).toContainText(/(?<![\d:])0:0[1-3]/);
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
	expect(await page.evaluate(() => (window as unknown as FakeWindow).__fake.voiceCount())).toBe(1);
});

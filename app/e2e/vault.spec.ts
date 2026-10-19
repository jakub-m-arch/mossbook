import type { Page } from '@playwright/test';

import { PASSWORD, SHOTS, createVault, expect, test } from './helpers';

/** Test hooks exposed by e2e/fake-tauri.js. */
type FakeWindow = Window & {
	__fake: { emit(event: string, payload: unknown): void; lockFromRust(reason: string): void };
};

/** Simulates an event sent by the Rust side. */
function emit(page: Page, event: string, payload: unknown) {
	return page.evaluate(
		([e, p]) => (window as FakeWindow).__fake.emit(e as string, p),
		[event, payload]
	);
}

test('create a vault with recovery key confirmation', async ({ page }) => {
	await createVault(page, true);
	await expect(page.getByRole('button', { name: 'Zablokuj' })).toBeVisible();
});

test('lock, wrong password with delay, unlock', async ({ page }) => {
	await createVault(page);
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();

	await page.getByLabel('Hasło', { exact: true }).fill('not it at all');
	await page.getByRole('button', { name: 'Otwórz' }).click();
	await expect(page.getByText('To hasło nie pasuje.')).toBeVisible();
	await expect(page.getByText(/Następna próba za \d s/)).toBeVisible();
	await expect(page.getByRole('button', { name: 'Otwórz' })).toBeDisabled();
	await page.screenshot({ path: `${SHOTS}/6-lock-wrong.png`, fullPage: true });

	await page.getByLabel('Hasło', { exact: true }).fill(PASSWORD);
	await expect(page.getByRole('button', { name: 'Otwórz' })).toBeEnabled({ timeout: 3000 });
	await page.getByRole('button', { name: 'Otwórz' }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
});

test('auto-lock from Rust returns to the lock screen', async ({ page }) => {
	await createVault(page);
	await page.evaluate(() => (window as FakeWindow).__fake.lockFromRust('idle'));
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
	// Going back in history must not show the journal.
	await page.goto('/timeline');
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
});

test('privacy curtain covers the content on focus loss', async ({ page }) => {
	await createVault(page);
	await emit(page, 'privacy-curtain', true);
	await expect(page.locator('.curtain')).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/7-curtain.png` });
	await emit(page, 'privacy-curtain', false);
	await expect(page.locator('.curtain')).toBeHidden();
});

test('recover with the recovery phrase', async ({ page }) => {
	await createVault(page);
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await page.getByRole('link', { name: 'Nie pamiętam hasła' }).click();
	await expect(page.getByRole('heading', { name: 'Odzyskaj dostęp' })).toBeVisible();
	const submit = page.getByRole('button', { name: 'Odzyskaj i ustaw hasło' });
	await page
		.getByLabel('Klucz odzyskiwania')
		.fill(Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join('\n'));
	await page.getByLabel('Nowe hasło').fill('a brand new password');
	await expect(submit).toBeDisabled();
	await page.getByLabel('Powtórz hasło').fill('a brand new password');
	await submit.click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
});

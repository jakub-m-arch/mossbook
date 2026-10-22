import type { Page } from '@playwright/test';

import { PASSWORD, SHOTS, createVault, expect, test } from './helpers';

/** Test hooks exposed by e2e/fake-tauri.js. */
type FakeWindow = Window & { __fake: { lockFromRust(reason: string): void } };

const NOTE = 'Synthetic words written just before the lock.';

/** Opens a new note through quick entry and types into it. */
async function startNote(page: Page) {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	await expect(page.getByRole('heading', { name: 'Notatka' })).toBeVisible();
	await page.getByLabel('Notatka', { exact: true }).fill(NOTE);
}

async function unlock(page: Page) {
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
	await page.getByLabel('Hasło', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Otwórz' }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
}

test.beforeEach(async ({ page }) => {
	await createVault(page);
});

test('an entry form survives an automatic lock and can be continued', async ({ page }) => {
	await startNote(page);
	await page.waitForTimeout(1200); // the form is kept once typing pauses
	await page.evaluate(() => (window as FakeWindow).__fake.lockFromRust('background'));
	await unlock(page);

	await expect(page.getByRole('heading', { name: 'Niezapisany wpis' })).toBeVisible();
	await expect(page.getByText(/^Notatka, .*Mossbook zachował go/)).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/30-unsaved-entry.png`, fullPage: true });
	await page.getByRole('button', { name: 'Wróć do wpisu' }).click();
	await expect(page.getByLabel('Notatka', { exact: true })).toHaveValue(NOTE);
	await page.getByRole('button', { name: 'Zapisz' }).click();

	await expect(page.getByRole('listitem').filter({ hasText: NOTE })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Niezapisany wpis' })).toBeHidden();
});

test('locking by hand right after typing keeps the form', async ({ page }) => {
	await startNote(page);
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await unlock(page);
	await page.getByRole('button', { name: 'Wróć do wpisu' }).click();
	await expect(page.getByLabel('Notatka', { exact: true })).toHaveValue(NOTE);
});

test('leaving the form on purpose discards it', async ({ page }) => {
	await startNote(page);
	await page.waitForTimeout(1200);
	await page.getByRole('link', { name: 'Wstecz' }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Niezapisany wpis' })).toBeHidden();
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await unlock(page);
	await expect(page.getByRole('heading', { name: 'Niezapisany wpis' })).toBeHidden();
});

test('a kept form can be discarded after a confirmation', async ({ page }) => {
	await startNote(page);
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await unlock(page);
	await page.getByRole('button', { name: 'Odrzuć' }).click();
	await expect(page.getByText('Odrzucić ten wpis? Tego nie da się cofnąć.')).toBeVisible();
	await page.getByRole('group').getByRole('button', { name: 'Odrzuć' }).click();
	await expect(page.getByRole('heading', { name: 'Niezapisany wpis' })).toBeHidden();
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await unlock(page);
	await expect(page.getByRole('heading', { name: 'Niezapisany wpis' })).toBeHidden();
});

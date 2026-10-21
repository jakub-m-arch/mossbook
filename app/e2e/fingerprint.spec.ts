import type { Page } from '@playwright/test';

import { PASSWORD, SHOTS, createVault, expect, test } from './helpers';

/** Test hooks exposed by e2e/fake-tauri.js. */
type FakeWindow = Window & { __fake: { setBiometric(next: string): void } };

/** What the next system fingerprint prompt does. */
function nextPrompt(page: Page, next: string) {
	return page.evaluate((n) => (window as FakeWindow).__fake.setBiometric(n), next);
}

async function enableFingerprint(page: Page) {
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	const section = page.getByRole('region', { name: 'Odblokowanie odciskiem palca' });
	await expect(section.getByText('Hasło i 24 słowa klucza odzyskiwania')).toBeVisible();
	await section.getByLabel('Wpisz hasło, żeby włączyć').fill('not my password');
	await section.getByRole('button', { name: 'Włącz odblokowanie odciskiem' }).click();
	await expect(section.getByText('To hasło nie pasuje.')).toBeVisible();
	await section.getByLabel('Wpisz hasło, żeby włączyć').fill(PASSWORD);
	await section.getByRole('button', { name: 'Włącz odblokowanie odciskiem' }).click();
	await expect(section.getByText('Odblokowanie odciskiem palca jest włączone.')).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/25-fingerprint-settings.png`, fullPage: true });
}

test.beforeEach(async ({ page }) => {
	await page.addInitScript(() => window.localStorage.setItem('fake-biometric', 'available'));
});

test('fingerprint: turn on in settings, unlock, fall back to the password', async ({ page }) => {
	await createVault(page);
	await enableFingerprint(page);

	// Cancelling the prompt leaves the password and the fingerprint button.
	await nextPrompt(page, 'cancelled');
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
	const button = page.getByRole('button', { name: 'Otwórz odciskiem palca' });
	await expect(button).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/26-lock-fingerprint.png`, fullPage: true });

	await nextPrompt(page, 'ok');
	await button.click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();

	// Turning it off: the lock screen asks for the password only.
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	const section = page.getByRole('region', { name: 'Odblokowanie odciskiem palca' });
	await section.getByRole('button', { name: 'Wyłącz odblokowanie odciskiem' }).click();
	await expect(section.getByText('Dziennik otworzysz hasłem.')).toBeVisible();
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Otwórz odciskiem palca' })).toHaveCount(0);
});

test('fingerprint: changed fingerprints turn it off and ask for the password', async ({ page }) => {
	await createVault(page);
	await enableFingerprint(page);
	await nextPrompt(page, 'device_key_invalidated');
	await page.getByRole('button', { name: 'Zablokuj' }).click();

	await expect(page.getByText('Odciski palców w telefonie się zmieniły')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Otwórz odciskiem palca' })).toHaveCount(0);
	await page.getByLabel('Hasło', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Otwórz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
});

test('fingerprint: a sensor without fingerprints gets a hint', async ({ page }) => {
	await page.addInitScript(() => window.localStorage.setItem('fake-biometric', 'enrollable'));
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	const section = page.getByRole('region', { name: 'Odblokowanie odciskiem palca' });
	await expect(section.getByText('nie ma zapisanego żadnego odcisku')).toBeVisible();
	await expect(section.getByRole('button')).toHaveCount(0);
});

test('fingerprint: the settings section is hidden without biometrics', async ({ page }) => {
	await page.addInitScript(() => window.localStorage.removeItem('fake-biometric'));
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	await expect(page.getByRole('heading', { name: 'Język' })).toBeVisible();
	await expect(page.getByRole('region', { name: 'Odblokowanie odciskiem palca' })).toHaveCount(0);
});

import { test as base, expect, type Page } from '@playwright/test';

export const PASSWORD = 'correct horse battery';
export const SHOTS = 'test-results/screens';

/**
 * `test` with a `page` that loads the fake backend and fails the test on any
 * uncaught error or console error (SvelteKit turns render errors into a 500 page).
 */
export const test = base.extend({
	page: async ({ page }, use) => {
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		page.on('console', (msg) => {
			if (msg.type() === 'error') errors.push(msg.text());
		});
		await page.addInitScript({ path: 'e2e/fake-tauri.js' });
		await use(page);
		expect(errors, 'errors in the page').toEqual([]);
	}
});

export { expect };

/** Walks through the setup flow (V-1) and ends on the timeline. */
export async function createVault(page: Page, shots = false) {
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Witaj w Mossbook' })).toBeVisible();
	await expect(page.getByText('nie zastępuje pomocy specjalisty')).toBeVisible();
	if (shots) await page.screenshot({ path: `${SHOTS}/1-welcome.png`, fullPage: true });
	await page.getByRole('button', { name: 'Zaczynam' }).click();

	const next = page.getByRole('button', { name: 'Dalej' });
	await page.getByLabel('Hasło', { exact: true }).fill('qwerty12');
	await expect(page.getByText('Siła hasła: słabe')).toBeVisible();
	await page.getByLabel('Hasło', { exact: true }).fill(PASSWORD);
	await page.getByLabel('Powtórz hasło').fill('different');
	await expect(page.getByText('Hasła się różnią.')).toBeVisible();
	await expect(next).toBeDisabled();
	await page.getByLabel('Powtórz hasło').fill(PASSWORD);
	if (shots) await page.screenshot({ path: `${SHOTS}/2-password.png`, fullPage: true });
	await next.click();

	await expect(page.getByRole('heading', { name: 'Twój klucz odzyskiwania' })).toBeVisible();
	await expect(
		page.getByText('Jeśli zgubisz hasło i ten klucz, nikt (także my) nie odzyska twoich wpisów.')
	).toBeVisible();
	await expect(page.getByRole('listitem')).toHaveCount(24);
	if (shots) await page.screenshot({ path: `${SHOTS}/3-phrase.png`, fullPage: true });
	// V-1: impossible to continue without confirming the key was stored.
	await expect(next).toBeDisabled();
	await page.getByLabel('Zapisałem/zapisałam klucz w bezpiecznym miejscu').check();
	await next.click();

	// The fake backend asks for words 3, 11 and 21 (positions 2, 10, 20).
	await page.getByLabel('Słowo nr 3').fill('word3');
	await page.getByLabel('Słowo nr 11').fill('word11');
	await page.getByLabel('Słowo nr 21').fill('wrong');
	await page.getByRole('button', { name: 'Załóż dziennik' }).click();
	await expect(page.getByText('Słowa się nie zgadzają.')).toBeVisible();
	if (shots) await page.screenshot({ path: `${SHOTS}/4-confirm-mismatch.png`, fullPage: true });
	await page.getByLabel('Słowo nr 21').fill('word21');
	await page.getByRole('button', { name: 'Załóż dziennik' }).click();

	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
	if (shots) await page.screenshot({ path: `${SHOTS}/5-timeline.png`, fullPage: true });
}

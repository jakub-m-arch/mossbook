import type { Page } from '@playwright/test';

import { createVault, expect, test } from './helpers';

/** A fear entry with a strategy and the intensity before and after it. */
async function fear(page: Page, strategy: string, before: number, after: number) {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Lęk / wyzwalacz' }).click();
	await page
		.getByRole('dialog')
		.getByRole('button', { name: String(before), exact: true })
		.click();
	await page.getByLabel('Co zastosowałem/zastosowałam').fill(strategy);
	await page
		.getByRole('group', { name: 'Intensywność po (0–10)' })
		.getByText(String(after), { exact: true })
		.click();
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
}

test('what helps: gathered from entries, worded as observations, added to the plan', async ({
	page
}) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Plan' }).click();
	await page.getByRole('link', { name: 'Co mi pomaga' }).click();
	await expect(page.getByText('pojawi się to tutaj', { exact: false })).toBeVisible();
	await page.getByRole('link', { name: 'Wstecz' }).click();

	await page.getByRole('link', { name: 'Dziennik' }).click();
	await fear(page, 'Synthetic breathing', 8, 4);
	await fear(page, 'synthetic  breathing', 6, 5);

	await page.getByRole('link', { name: 'Plan' }).click();
	await page.getByRole('link', { name: 'Co mi pomaga' }).click();
	const item = page.getByRole('listitem').filter({ hasText: /breathing/i });
	await expect(item).toHaveCount(1);
	await expect(item).toContainText('Wpisy: 2');
	await expect(item).toContainText('Lęk po: średnio o 2,5 niżej');

	await item.getByRole('button', { name: 'Dodaj do planu' }).click();
	await expect(item).toContainText('W planie');
	await page.getByRole('link', { name: 'Wstecz' }).click();
	const own = page.getByRole('group', { name: 'Co mogę zrobić sam/sama' });
	await expect(own.getByRole('textbox')).toHaveValue(/synthetic\s+breathing/i);
});

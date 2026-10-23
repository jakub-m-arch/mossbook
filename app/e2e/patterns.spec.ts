import type { Page } from '@playwright/test';

import { SHOTS, createVault, expect, openSettings, test } from './helpers';

async function quickMood(page: Page, label: string) {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Samopoczucie' }).click();
	await page.getByRole('dialog').getByRole('button', { name: label, exact: true }).click();
	await page.getByRole('link', { name: 'Wstecz' }).click();
}

test('mood chart with tooltip and table view', async ({ page }) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Wzorce' }).click();
	await expect(page.getByText('W tym okresie nie ma jeszcze ocen samopoczucia.')).toBeVisible();

	await page.getByRole('link', { name: 'Dziennik' }).click();
	await quickMood(page, 'dobrze');
	await quickMood(page, 'ciężko');
	await page.getByRole('link', { name: 'Wzorce' }).click();

	await expect(page.getByText('Średnia: 3 (dni z oceną: 1).')).toBeVisible();
	const chart = page.getByRole('slider', { name: 'Samopoczucie w czasie' });
	await chart.focus();
	await chart.press('ArrowLeft');
	await expect(chart).toHaveAttribute('aria-valuetext', /średnio 3, ocen: 2/);
	await page.screenshot({ path: `${SHOTS}/18-patterns.png`, fullPage: true });

	await page.getByText('Pokaż jako tabelę').click();
	await expect(page.getByRole('row')).toHaveCount(2);
	await page.getByRole('radio', { name: 'Tydzień' }).click();
	await expect(page.getByRole('radio', { name: 'Tydzień' })).toHaveAttribute(
		'aria-checked',
		'true'
	);
});

test('language can be chosen in settings', async ({ page }) => {
	await createVault(page);
	await openSettings(page, 'Język i blokada');
	await page.getByLabel('English').check();
	await expect(page.getByRole('heading', { name: 'Language and lock', exact: true })).toBeVisible();
	await page.getByLabel('Українська').check();
	await expect(page.getByRole('heading', { name: 'Мова і блокування' })).toBeVisible();
	await page.getByLabel('Як у системі').check();
	await expect(page.getByRole('heading', { name: 'Język i blokada' })).toBeVisible();
	await page.getByRole('link', { name: 'Wstecz' }).click();
	await expect(page.getByRole('link', { name: 'Dziennik' })).toBeVisible();
});

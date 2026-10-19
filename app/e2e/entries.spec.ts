import type { Page } from '@playwright/test';

import { SHOTS, createVault, expect, test } from './helpers';

test.beforeEach(async ({ page }) => {
	await createVault(page);
});

const card = (page: Page, text: string) => page.getByRole('listitem').filter({ hasText: text });
const dialog = (page: Page) => page.getByRole('dialog');

test('quick entry takes at most two taps', async ({ page }) => {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await expect(dialog(page)).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/8-quick-kind.png` });
	await dialog(page).getByRole('button', { name: 'Lęk / wyzwalacz' }).click(); // tap 1
	await page.screenshot({ path: `${SHOTS}/9-quick-value.png` });
	await dialog(page).getByRole('button', { name: '7', exact: true }).click(); // tap 2
	await expect(dialog(page)).toBeHidden();
	await expect(page.getByText('Zapisano. Możesz uzupełnić później.')).toBeVisible();
	const fear = card(page, 'Lęk / wyzwalacz');
	await expect(fear).toContainText('7/10');
	await expect(fear).toContainText('do uzupełnienia');

	// Mood: rating 1–5 with pictograms; a note is saved with a single tap.
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await dialog(page).getByRole('button', { name: 'Samopoczucie' }).click();
	await dialog(page).getByRole('button', { name: 'dobrze', exact: true }).click();
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await dialog(page).getByRole('button', { name: 'Notatka' }).click();
	await expect(page.getByRole('listitem')).toHaveCount(3);
	await page.screenshot({ path: `${SHOTS}/10-timeline.png`, fullPage: true });
});

test('full fear form, edit and delete', async ({ page }) => {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await dialog(page).getByRole('button', { name: 'Lęk / wyzwalacz' }).click();
	await dialog(page).getByRole('link', { name: 'Więcej szczegółów' }).click();

	await page.getByLabel('Co to wywołało').fill('tłum w tramwaju');
	await page
		.getByRole('group', { name: 'Intensywność (0–10)' })
		.getByText('8', { exact: true })
		.click();
	await page.getByRole('button', { name: 'kołatanie serca' }).click();
	await page.getByLabel('Objawy w ciele: Dodaj własne').fill('zimne dłonie');
	await page.getByLabel('Objawy w ciele: Dodaj własne').press('Enter');
	await expect(page.getByRole('button', { name: 'zimne dłonie ×' })).toBeVisible();
	await page.getByLabel('Co zastosowałem/zastosowałam').fill('oddech pudełkowy');
	await page
		.getByRole('group', { name: 'Intensywność po (0–10)' })
		.getByText('3', { exact: true })
		.click();
	await page.screenshot({ path: `${SHOTS}/11-fear-form.png`, fullPage: true });
	await page.getByRole('button', { name: 'Zapisz' }).click();

	const fear = card(page, 'tłum w tramwaju');
	await expect(fear).toContainText('oddech pudełkowy');
	await expect(fear).toContainText('8/10');
	await expect(fear).not.toContainText('do uzupełnienia');

	await fear.getByRole('link').click();
	await expect(page.getByLabel('Co to wywołało')).toHaveValue('tłum w tramwaju');
	await expect(page.getByRole('button', { name: 'kołatanie serca' })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.getByLabel('Notatka').fill('potem było lepiej');
	await page.getByRole('button', { name: 'Zapisz' }).click();
	await expect(card(page, 'tłum w tramwaju')).toContainText('potem było lepiej');

	await card(page, 'tłum w tramwaju').getByRole('link').click();
	await page.getByRole('button', { name: 'Usuń wpis' }).click();
	await expect(page.getByText('Usunąć ten wpis? Tego nie da się cofnąć.')).toBeVisible();
	await page
		.getByRole('group', { name: 'Usunąć ten wpis?' })
		.getByRole('button', { name: 'Usuń wpis' })
		.click();
	await expect(page.getByText('Tu pojawią się twoje wpisy.')).toBeVisible();
});

test('tags, tag filter and diacritic-insensitive search', async ({ page }) => {
	await page.getByRole('link', { name: 'Tagi' }).click();
	await page.getByLabel('Nazwa').fill('Praca');
	await page.getByRole('button', { name: 'Dodaj tag' }).click();
	await page.getByLabel('Nazwa').first().fill('praca');
	await page.getByRole('button', { name: 'Dodaj tag' }).click();
	await expect(page.getByText('Tag o tej nazwie już istnieje.')).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/12-tags.png`, fullPage: true });
	await page.getByRole('link', { name: 'Wstecz' }).click();

	// A note in Łódź with the tag, and one without.
	for (const [note, tagged] of [
		['Spacer po Łodzi', true],
		['Zakupy', false]
	] as const) {
		await page.getByRole('button', { name: 'Szybki wpis' }).click();
		await dialog(page).getByRole('button', { name: 'Lęk / wyzwalacz' }).click();
		await dialog(page).getByRole('link', { name: 'Więcej szczegółów' }).click();
		await page.getByLabel('Notatka').fill(note);
		if (tagged) await page.getByRole('button', { name: 'Praca' }).click();
		await page.getByRole('button', { name: 'Zapisz' }).click();
		await expect(card(page, note)).toBeVisible();
	}

	await page.getByRole('group', { name: 'Tagi' }).getByRole('button', { name: 'Praca' }).click();
	await expect(page.getByRole('listitem')).toHaveCount(1);
	await page.getByRole('button', { name: 'Wyczyść filtry' }).click();
	await expect(page.getByRole('listitem')).toHaveCount(2);

	await page.getByPlaceholder('Szukaj').fill('lodzi');
	await expect(page.getByRole('listitem')).toHaveCount(1);
	await expect(card(page, 'Spacer po Łodzi')).toBeVisible();
	await page.getByPlaceholder('Szukaj').fill('nic takiego');
	await expect(page.getByText('Nic nie pasuje do filtrów.')).toBeVisible();
});

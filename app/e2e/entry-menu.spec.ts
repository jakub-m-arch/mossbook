import type { Locator } from '@playwright/test';

import { createVault, expect, test } from './helpers';

/** A finger held on the card, as a touch pointer (Chromium only fakes the pointer events). */
async function longPress(target: Locator) {
	const box = (await target.boundingBox())!;
	const point = {
		pointerType: 'touch',
		isPrimary: true,
		clientX: box.x + box.width / 2,
		clientY: box.y + box.height / 2
	};
	await target.dispatchEvent('pointerdown', point);
	await target.page().waitForTimeout(600);
	await target.dispatchEvent('pointerup', point);
}

test('new entries hide their content; the entry menu changes that and deletes', async ({
	page
}) => {
	await createVault(page);
	await page.goto('/entry/new?kind=note');
	await expect(page.getByLabel('Ukrywanie treści')).toBeChecked();
	await page.getByLabel('Notatka', { exact: true }).fill('Synthetic walk by the river');
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();

	const card = page.getByRole('listitem').filter({ hasText: 'Notatka' });
	await expect(card).toContainText('Treść ukryta');
	await expect(card).not.toContainText('river');

	// Holding the entry opens its menu, not the entry.
	await longPress(card.getByRole('link'));
	const menu = page.getByRole('dialog', { name: /Notatka ·/ });
	await expect(menu).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
	await menu.getByRole('button', { name: 'Nie ukrywaj treści' }).click();
	await expect(card).toContainText('Synthetic walk by the river');

	// Right click (desktop) opens the same menu; deleting asks first.
	await card.getByRole('link').click({ button: 'right' });
	await expect(menu.getByRole('button', { name: 'Ukrywaj treść' })).toBeVisible();
	await menu.getByRole('button', { name: 'Usuń wpis' }).click();
	const confirm = page.getByRole('dialog', { name: 'Usunąć ten wpis?' });
	await expect(confirm.getByText('Tego nie da się cofnąć.')).toBeVisible();
	await expect(confirm.getByRole('button', { name: 'Anuluj' })).toBeFocused();
	await confirm.getByRole('button', { name: 'Usuń wpis' }).click();
	await expect(page.getByText('Tu pojawią się twoje wpisy.')).toBeVisible();
});

test('a quick entry, a single number, is not hidden', async ({ page }) => {
	await createVault(page);
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Samopoczucie' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'dobrze', exact: true }).click();
	await expect(page.getByLabel('Ukrywanie treści')).not.toBeChecked();
	await page.getByRole('link', { name: 'Wstecz' }).click();
	await expect(page.getByRole('listitem').filter({ hasText: 'Samopoczucie' })).not.toContainText(
		'Treść ukryta'
	);
});

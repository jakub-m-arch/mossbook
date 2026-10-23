import { PASSWORD, SHOTS, createVault, expect, test } from './helpers';

test('nightmare with a rescripted version, hidden on the timeline', async ({ page }) => {
	await createVault(page);
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Koszmar' }).click(); // one tap

	await expect(page.getByRole('heading', { name: 'Koszmar' })).toBeVisible();
	await page.getByLabel('Treść koszmaru').fill('syntetyczny koszmar o ciemnym korytarzu');
	await page.getByLabel('Nowa wersja').fill('korytarz się rozjaśnia i wychodzę do ogrodu');
	await page.getByRole('button', { name: 'Ćwiczyłem/ćwiczyłam dziś' }).click();
	await expect(page.getByText('Jeszcze bez powtórek.')).toBeHidden();
	await expect(page.getByLabel('Ukrywanie treści')).toBeChecked();
	await page.screenshot({ path: `${SHOTS}/20-nightmare.png`, fullPage: true });
	await page.getByRole('button', { name: 'Zapisz' }).click();

	// Hidden on the timeline: only the type, the time and a mark; tapping opens it.
	const card = page.getByRole('listitem').filter({ hasText: 'Koszmar' });
	await expect(card).toContainText('Treść ukryta');
	await expect(card).not.toContainText('ogrodu');
	await expect(card.getByRole('button')).toHaveCount(0);

	// Reopened, the nightmare itself starts collapsed.
	await card.getByRole('link').click();
	await expect(page.getByLabel('Treść koszmaru')).toBeHidden();
	await page.getByRole('button', { name: 'Pokaż treść koszmaru' }).click();
	await expect(page.getByLabel('Treść koszmaru')).toHaveValue(/ciemnym korytarzu/);
});

test('calendar tints rated days and opens a day', async ({ page }) => {
	await createVault(page);
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Samopoczucie' }).click();
	await page
		.getByRole('dialog')
		.getByRole('button', { name: 'bardzo dobrze', exact: true })
		.click();
	await page.getByRole('link', { name: 'Wstecz' }).click();

	await page.getByRole('link', { name: 'Wzorce' }).click();
	const today = page.getByRole('button', { name: /: średnio 5$/ });
	await expect(today).toBeVisible();
	await page.waitForTimeout(300); // let the tint transition finish before the screenshot
	await page.screenshot({ path: `${SHOTS}/21-calendar.png`, fullPage: true });
	await today.click();
	await expect(page.getByText(/^Wpisy z dnia/)).toBeVisible();
	await expect(page.getByRole('listitem')).toHaveCount(1);
	await page.getByRole('button', { name: 'Wyczyść filtry' }).click();
	await expect(page.getByText(/^Wpisy z dnia/)).toBeHidden();
});

test('quick entry from the lock screen arrives after unlocking', async ({ page }) => {
	await createVault(page);
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await expect(page.getByText('Możesz dodać wpis bez hasła.')).toBeVisible();
	await page.getByRole('dialog').getByRole('button', { name: 'Lęk / wyzwalacz' }).click();
	await page.getByRole('dialog').getByRole('button', { name: '5', exact: true }).click();
	await expect(
		page.getByText('Zapisano. Wpis pojawi się w dzienniku po odblokowaniu.')
	).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/23-locked-quick.png`, fullPage: true });

	await page.getByLabel('Hasło', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Otwórz' }).click();
	const card = page.getByRole('listitem').filter({ hasText: 'Lęk / wyzwalacz' });
	await expect(card).toContainText('5/10');
	await expect(card).toContainText('do uzupełnienia');
});

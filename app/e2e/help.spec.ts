import type { Page } from '@playwright/test';

import { SHOTS, createVault, expect, test } from './helpers';

type FakeWindow = Window & { __fake: { calls: string[] } };
const calls = (page: Page) => page.evaluate(() => (window as FakeWindow).__fake.calls);

test('crisis numbers and exercises are reachable from the lock screen', async ({ page }) => {
	await createVault(page);
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await page.getByRole('link', { name: 'Plan na trudny moment / telefony' }).click();

	await expect(page.getByRole('heading', { name: 'Gdy jest trudno' })).toBeVisible();
	for (const number of ['112', '116 123', '800 70 2222', '116 111']) {
		await expect(page.getByRole('button', { name: `Zadzwoń ${number}` })).toBeVisible();
	}
	await expect(page.getByText(/Sprawdzone \d{4}-\d{2}-\d{2}/)).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/13-help.png`, fullPage: true });

	await page.getByRole('button', { name: 'Zadzwoń 116 123' }).click();
	expect(await calls(page)).toEqual(['116123']);

	// Still locked: no user data on this screen.
	await page.getByRole('link', { name: 'Oddech pudełkowy' }).click();
	await expect(page.getByRole('heading', { name: 'Oddech pudełkowy' })).toBeVisible();
	await page.getByRole('button', { name: 'Zacznij' }).click();
	await expect(page.getByText('Wdech', { exact: true })).toBeVisible();
	await expect(page.getByText('Cykl 1 z 4')).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/14-breathing.png` });
	await page.getByRole('button', { name: 'Zatrzymaj' }).click();
	await page.getByRole('button', { name: '← Wstecz' }).click();

	await page.getByRole('link', { name: 'Technika 5-4-3-2-1' }).click();
	await expect(page.getByText('Nazwij 5 rzeczy, które widzisz.')).toBeVisible();
	for (let i = 0; i < 5; i++) await page.getByRole('button', { name: 'Dalej' }).click();
	await expect(page.getByText('Dobrze. Zostań chwilę z tym, jak się teraz czujesz.')).toBeVisible();

	await page.getByRole('button', { name: '← Wstecz' }).click();
	await page.getByRole('button', { name: '← Wstecz' }).click();
	await expect(page.getByRole('heading', { name: 'Dziennik jest zablokowany' })).toBeVisible();
});

test('safety plan: edit, save, call a contact', async ({ page }) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Plan na trudny moment' }).click();
	await expect(page.getByRole('heading', { name: 'Plan na trudny moment' })).toBeVisible();

	const signs = page.getByRole('group', { name: 'Moje sygnały ostrzegawcze' });
	await signs.getByRole('button', { name: 'Dodaj' }).click();
	await signs.getByLabel('Punkt 1').fill('nie mogę zasnąć');
	const contacts = page.getByRole('group', { name: 'Do kogo mogę zadzwonić' });
	await contacts.getByRole('button', { name: 'Dodaj' }).click();
	await contacts.getByLabel('Imię lub nazwa').fill('Ola');
	await contacts.getByLabel('Telefon').fill('+48 600 000 000');

	// Unsaved contacts cannot be dialed: only saved plan numbers are allowed.
	await contacts.getByRole('button', { name: 'Zadzwoń +48 600 000 000' }).click();
	await expect(page.getByText('Nie udało się rozpocząć połączenia.')).toBeVisible();

	await page.getByRole('button', { name: 'Zapisz plan' }).click();
	await expect(page.getByText('Plan zapisany.')).toBeVisible();
	await contacts.getByRole('button', { name: 'Zadzwoń +48 600 000 000' }).click();
	expect(await calls(page)).toEqual(['+48600000000']);
	await page.screenshot({ path: `${SHOTS}/15-plan.png`, fullPage: true });

	// Leave and come back through the app: the saved plan is loaded again.
	await page.getByRole('link', { name: 'Wstecz' }).click();
	await page.getByRole('link', { name: 'Plan na trudny moment' }).click();
	await expect(
		page.getByRole('group', { name: 'Moje sygnały ostrzegawcze' }).getByLabel('Punkt 1')
	).toHaveValue('nie mogę zasnąć');
});

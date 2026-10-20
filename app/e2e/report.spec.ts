import { SHOTS, createVault, expect, test } from './helpers';

type FakeWindow = Window & {
	__fake: { reports: Array<{ sections: Array<{ items: Array<{ title: string }> }> }> };
};

test('report: choose, preview, warning, save', async ({ page }) => {
	await createVault(page);
	// Two entries: a fear with details and a note.
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Lęk / wyzwalacz' }).click();
	await page.getByRole('dialog').getByRole('button', { name: '6', exact: true }).click();
	await page.getByLabel('Co to wywołało').fill('rozmowa telefoniczna');
	await page.getByRole('button', { name: 'Zapisz' }).click();
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	await page.getByRole('link', { name: 'Wstecz' }).click();

	await page.getByRole('link', { name: 'Ustawienia' }).click();
	await page.getByRole('link', { name: 'Raport dla terapeuty' }).click();
	await expect(page.getByRole('heading', { name: 'Raport dla terapeuty' })).toBeVisible();

	const preview = page.getByRole('article', { name: 'Mossbook — moje zapiski' });
	await expect(preview).toContainText('Liczba wpisów: 2');
	await expect(preview).toContainText('rozmowa telefoniczna');
	await expect(preview).toContainText('Intensywność: 6/10');
	await expect(page.getByText('Plik PDF nie jest zaszyfrowany.')).toBeVisible();

	// Narrow down to one type.
	await page
		.getByRole('group', { name: 'Rodzaje wpisów' })
		.getByRole('button', { name: 'Notatka' })
		.click();
	await expect(preview).toContainText('Liczba wpisów: 1');
	await page.screenshot({ path: `${SHOTS}/19-report.png`, fullPage: true });

	await page.getByRole('button', { name: 'Zapisz PDF' }).click();
	await expect(page.getByText('Raport zapisany.')).toBeVisible();
	const reports = await page.evaluate(() => (window as FakeWindow).__fake.reports);
	expect(reports).toHaveLength(1);
	expect(reports[0].sections.flatMap((s) => s.items.map((i) => i.title))).toEqual([
		expect.stringMatching(/Lęk \/ wyzwalacz$/)
	]);
});

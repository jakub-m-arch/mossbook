import { createVault, expect, SHOTS, test } from './helpers';

type FakeWindow = Window & { __fake: { seed(entries: unknown[]): void } };

/** Synthetic fear entries with a strategy, spread over recent mornings and evenings. */
function seedEntries(count: number) {
	const now = Date.now();
	return Array.from({ length: count }, (_, i) => {
		const at = new Date(now - (i + 1) * 3600_000 * 13).toISOString();
		return {
			id: `seed-${i}`,
			schema: 1,
			created_at: at,
			updated_at: at,
			occurred_at: at,
			kind: {
				fear: {
					trigger: 'syntetyczny wyzwalacz',
					symptoms: [],
					avoidance: null,
					strategy: i % 2 ? 'oddech pudełkowy' : 'spacer',
					intensity_after: 3
				}
			},
			mood: (i % 5) + 1,
			intensity: 7,
			tags: [],
			note: '',
			incomplete: false,
			sensitive: false
		};
	});
}

test('overviews appear from 14 entries, as observations', async ({ page }) => {
	await createVault(page);
	await page.evaluate((e) => (window as FakeWindow).__fake.seed(e), seedEntries(13));
	await page.getByRole('link', { name: 'Wzorce' }).click();
	await expect(page.getByText('co najmniej 14 wpisów (teraz: 13)')).toBeVisible();

	await page.evaluate((e) => (window as FakeWindow).__fake.seed(e), seedEntries(14).slice(13));
	await page.getByRole('radio', { name: 'Tydzień' }).click();
	await page.getByRole('radio', { name: 'Miesiąc', exact: true }).click();

	const strategies = page.getByRole('list', { name: 'Co pomagało przy lęku' });
	await expect(strategies).toContainText('oddech pudełkowy');
	await expect(strategies).toContainText('spadek średnio o 4');
	await expect(page.getByRole('list', { name: 'Samopoczucie a pora dnia' })).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/22-insights.png`, fullPage: true });
});

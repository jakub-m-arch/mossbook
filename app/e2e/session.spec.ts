import type { Page } from '@playwright/test';

import { createVault, expect, test } from './helpers';

/** Test hooks exposed by e2e/fake-tauri.js. */
type FakeWindow = Window & {
	__fake: { reports: Array<{ sections: Array<{ heading: string; items: unknown[] }> }> };
};

/** A note started by quick entry and completed in the form. */
async function note(page: Page, text: string, { discuss = false, hidden = true } = {}) {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	await page.getByLabel('Notatka', { exact: true }).fill(text);
	await page.getByLabel('Ukrywanie treści').setChecked(hidden);
	if (discuss) await page.getByLabel('Na sesję').check();
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
}

test('session: mark topics, add a question, write the session, and what waits', async ({
	page
}) => {
	await createVault(page);
	// Quick entry does not offer a session.
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await expect(page.getByRole('dialog').getByRole('button', { name: 'Sesja' })).toHaveCount(0);
	await page.keyboard.press('Escape');

	const card = page.getByRole('link', { name: /Na następną sesję/ });
	await note(page, 'Synthetic walk by the river', { hidden: false });
	await expect(card).toHaveCount(0);
	await note(page, 'Synthetic hidden topic', { discuss: true });
	await expect(card).toContainText('Tematy: 1');

	// The entry menu marks another one.
	const walk = page.getByRole('listitem').filter({ hasText: 'Synthetic walk' });
	await walk.getByRole('link').click({ button: 'right' });
	await page.getByRole('dialog').getByRole('button', { name: 'Na sesję' }).click();
	await expect(walk).toContainText('Na sesję');
	await expect(card).toContainText('Tematy: 2');

	await card.click();
	await expect(page.getByRole('heading', { name: 'Na następną sesję' })).toBeVisible();
	const topics = page.getByRole('region', { name: 'Tematy' });
	await expect(topics.getByRole('listitem')).toHaveCount(2);

	// A question is a note marked for the session.
	await page.getByRole('link', { name: 'Dodaj pytanie' }).click();
	await expect(page.getByLabel('Na sesję')).toBeChecked();
	await page.getByLabel('Notatka', { exact: true }).fill('Synthetic question about sleep');
	await page.getByLabel('Ukrywanie treści').uncheck();
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Na następną sesję' })).toBeVisible();
	await expect(topics.getByRole('listitem')).toHaveCount(3);

	// After the session: tick what was talked about.
	await page.getByRole('link', { name: 'Zapisz notatkę po sesji' }).click();
	await expect(page.getByRole('heading', { name: 'Sesja' })).toBeVisible();
	await expect(page.getByLabel('Na sesję')).toHaveCount(0);
	const talked = page.getByRole('group', { name: 'O czym rozmawialiście?' });
	await expect(talked.getByRole('checkbox')).toHaveCount(3);
	// Hidden content stays hidden in the list.
	await expect(talked).not.toContainText('hidden topic');
	await talked.getByLabel(/Synthetic walk/).check();
	await talked.getByLabel(/Synthetic question/).check();
	await page.getByLabel('Co ustaliliśmy').fill('Synthetic agreement: shorter evenings');
	await page
		.getByLabel('Co mam zrobić do następnej sesji')
		.fill('Synthetic homework: note the evenings');
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();

	await expect(page.getByRole('heading', { name: 'Na następną sesję' })).toBeVisible();
	await expect(page.getByRole('region', { name: 'Do zrobienia z ostatniej sesji' })).toContainText(
		'Synthetic homework: note the evenings'
	);
	await expect(topics.getByRole('listitem')).toHaveCount(1, { timeout: 5000 });

	// The rest waits; taking it off the list empties it.
	await topics.getByRole('button', { name: 'Zdejmij z listy' }).click();
	await expect(topics.getByText('Nic jeszcze nie jest oznaczone.', { exact: false })).toBeVisible();

	// The timeline still points to the to-do; the session is a searchable entry.
	await page.getByRole('link', { name: 'Wstecz' }).click();
	await expect(card).toContainText('Do zrobienia z ostatniej sesji');
	await page.getByPlaceholder('Szukaj').fill('evenings');
	await expect(page.getByRole('listitem').filter({ hasText: 'Sesja' })).toHaveCount(1);
});

test('session: the report can start with the topics, whatever the period', async ({ page }) => {
	await createVault(page);
	await note(page, 'Synthetic old topic', { discuss: true, hidden: false });
	// Moved out of the report's default period (the last 30 days).
	const entry = page.getByRole('listitem').filter({ hasText: 'Synthetic old topic' });
	await entry.getByRole('link').click();
	await page.getByLabel('Kiedy').fill('2025-01-15T10:00');
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await note(page, 'Synthetic recent note', { hidden: false });

	await page.getByRole('link', { name: 'Ustawienia' }).click();
	await page.getByRole('link', { name: 'Raport dla terapeuty' }).click();
	const preview = page.getByRole('article', { name: 'Mossbook — moje zapiski' });
	const lead = page.getByLabel('Zacznij od tematów na sesję');
	await expect(lead).toBeChecked();
	await expect(preview).toContainText('Tematy na tę sesję');
	await expect(preview).toContainText('Synthetic old topic');
	await expect(preview).toContainText('Synthetic recent note');

	await page.getByRole('button', { name: 'Zapisz PDF' }).click();
	await expect(page.getByText('Raport zapisany.')).toBeVisible();
	const reports = await page.evaluate(() => (window as unknown as FakeWindow).__fake.reports);
	expect(reports[0].sections[0].heading).toBe('Tematy na tę sesję');

	await lead.uncheck();
	await expect(preview).not.toContainText('Tematy na tę sesję');
	await expect(preview).not.toContainText('Synthetic old topic');
});

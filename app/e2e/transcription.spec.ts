import type { Page } from '@playwright/test';

import { createVault, expect, test } from './helpers';

/** Test hooks exposed by e2e/fake-tauri.js. */
type FakeWindow = Window & {
	__fake: {
		downloadsOpened: number;
		setTranscription(options: Record<string, unknown>): void;
	};
};

const fake = (page: Page) => page.evaluate(() => (window as unknown as FakeWindow).__fake);

async function newNoteWithRecording(page: Page) {
	await page.getByRole('button', { name: 'Szybki wpis' }).click();
	await page.getByRole('dialog').getByRole('button', { name: 'Notatka' }).click();
	const voice = page.getByRole('region', { name: 'Notatka głosowa' });
	await voice.getByRole('button', { name: 'Nagraj notatkę głosową' }).click();
	await expect(voice.getByRole('status')).toContainText('Nagrywanie');
	await page.waitForTimeout(1200);
	await voice.getByRole('button', { name: 'Zakończ nagrywanie' }).click();
	await expect(voice.getByRole('button', { name: 'Odtwórz notatkę głosową' })).toBeVisible();
	return voice;
}

test('transcription is off by default; the model is downloaded in the browser and imported', async ({
	page
}) => {
	await createVault(page);
	// Off: a recording offers no transcription.
	const voice = await newNoteWithRecording(page);
	await expect(voice.getByRole('button', { name: 'Zamień na tekst' })).toHaveCount(0);
	await page.getByLabel('Notatka', { exact: true }).fill('Synthetic note');
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await page.getByRole('link', { name: 'Ustawienia' }).click();

	const section = page.getByRole('region', { name: 'Zamiana nagrań na tekst' });
	await expect(section.getByText('Domyślnie ta funkcja jest wyłączona.')).toBeVisible();
	await expect(section.getByText(/487 MB do pobrania, 670 MB na dysku/)).toBeVisible();
	await expect(section.getByText(/Mossbook sam niczego nie pobiera/)).toBeVisible();
	await expect(section.getByText(/CC BY 4\.0/)).toBeVisible();
	await expect(section.getByRole('note')).toHaveCount(0);

	await section.getByRole('button', { name: 'Otwórz stronę pobierania' }).click();
	await expect.poll(async () => (await fake(page)).downloadsOpened).toBe(1);

	// A wrong file is refused; a closed dialog changes nothing.
	await page.evaluate(() =>
		(window as unknown as FakeWindow).__fake.setTranscription({ nextImport: 'invalid' })
	);
	await section.getByRole('button', { name: 'Wskaż pobrany plik' }).click();
	await expect(section.getByText(/To nie jest oczekiwany plik modelu/)).toBeVisible();
	await page.evaluate(() =>
		(window as unknown as FakeWindow).__fake.setTranscription({ nextImport: 'cancel' })
	);
	await section.getByRole('button', { name: 'Wskaż pobrany plik' }).click();
	await expect(section.getByRole('button', { name: 'Wskaż pobrany plik' })).toBeEnabled();

	await page.evaluate(() =>
		(window as unknown as FakeWindow).__fake.setTranscription({ nextImport: 'ok' })
	);
	await section.getByRole('button', { name: 'Wskaż pobrany plik' }).click();
	await expect(section.getByText('Gotowe. Zamiana na tekst jest włączona.')).toBeVisible();
	await expect(section.getByText(/Włączona\. Model zajmuje 670 MB/)).toBeVisible();

	// Now the saved recording offers it.
	await page.getByRole('link', { name: 'Dziennik' }).click();
	await page.getByRole('listitem').filter({ hasText: 'Synthetic note' }).getByRole('link').click();
	await expect(voice.getByRole('button', { name: 'Zamień na tekst' })).toBeVisible();
	await page.goBack();
	await page.getByRole('link', { name: 'Ustawienia' }).click();

	// Removing the model turns it off again.
	await section.getByRole('button', { name: 'Usuń model i wyłącz' }).click();
	await section
		.getByRole('dialog', { name: /Usunąć model/ })
		.getByRole('button', { name: 'Usuń model i wyłącz' })
		.click();
	await expect(section.getByText('Model usunięty.', { exact: false })).toBeVisible();
	await expect(section.getByRole('button', { name: 'Wskaż pobrany plik' })).toBeVisible();
});

test('transcription: a warning on a device with little memory', async ({ page }) => {
	await page.addInitScript(() => window.localStorage.setItem('fake-low-memory', 'yes'));
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	const section = page.getByRole('region', { name: 'Zamiana nagrań na tekst' });
	await expect(section.getByRole('note')).toContainText('To urządzenie ma 3,9 GB pamięci.');
});

test('transcription: turn a recording into text, correct it, save and find it', async ({
	page
}) => {
	await page.addInitScript(() => window.localStorage.setItem('fake-transcription', 'installed'));
	await createVault(page);
	const voice = await newNoteWithRecording(page);

	await voice.getByRole('button', { name: 'Zamień na tekst' }).click();
	await expect(voice.getByRole('status')).toContainText(/Zamieniam na tekst… \d z 2/);
	const text = voice.getByLabel('Tekst nagrania');
	await expect(text).toHaveValue('Synthetic transcript: a calm walk by the river.');
	await expect(voice.getByText(/Może zawierać błędy/)).toBeVisible();

	// The user corrects it before saving.
	await text.fill('Synthetic transcript: a calm walk by the lake.');
	await page.getByRole('button', { name: 'Zapisz', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();

	// Searchable like any other text.
	await page.getByPlaceholder('Szukaj').fill('lake');
	const card = page.getByRole('listitem').filter({ hasText: /0:0\d/ });
	await expect(card).toHaveCount(1);
	await page.getByPlaceholder('Szukaj').fill('river');
	await expect(card).toHaveCount(0);
	await page.getByPlaceholder('Szukaj').fill('');

	await card.getByRole('link').click();
	await expect(voice.getByLabel('Tekst nagrania')).toHaveValue(
		'Synthetic transcript: a calm walk by the lake.'
	);
	await expect(voice.getByRole('button', { name: 'Zamień na tekst' })).toHaveCount(0);
});

test('transcription: can be stopped, and nothing is invented for silence', async ({ page }) => {
	await page.addInitScript(() => window.localStorage.setItem('fake-transcription', 'installed'));
	await createVault(page);
	const voice = await newNoteWithRecording(page);
	await page.evaluate(() =>
		(window as unknown as FakeWindow).__fake.setTranscription({ delayMs: 2000 })
	);
	await voice.getByRole('button', { name: 'Zamień na tekst' }).click();
	await voice.getByRole('button', { name: 'Przerwij' }).click();
	await expect(voice.getByRole('button', { name: 'Zamień na tekst' })).toBeVisible();
	await expect(voice.getByLabel('Tekst nagrania')).toHaveCount(0);

	await page.evaluate(() =>
		(window as unknown as FakeWindow).__fake.setTranscription({ delayMs: 10, text: '' })
	);
	await voice.getByRole('button', { name: 'Zamień na tekst' }).click();
	await expect(voice.getByText('Nie rozpoznano słów w tym nagraniu.')).toBeVisible();
	await expect(voice.getByLabel('Tekst nagrania')).toHaveCount(0);
});

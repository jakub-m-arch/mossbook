import { PASSWORD, SHOTS, createVault, expect, test } from './helpers';

test('restore a backup on a new device', async ({ page }) => {
	await page.goto('/');
	await page.getByRole('link', { name: 'Mam kopię zapasową' }).click();
	await expect(page.getByRole('heading', { name: 'Przywróć z kopii' })).toBeVisible();
	await page.getByRole('button', { name: 'Wybierz plik' }).click();
	await expect(page.getByText('Liczba zapisanych elementów w kopii: 3.')).toBeVisible();

	await page.getByLabel('Hasło do kopii').fill('not the one');
	await page.getByRole('button', { name: 'Wczytaj' }).click();
	await expect(page.getByText('To hasło nie pasuje.')).toBeVisible();
	await page.getByLabel('Hasło do kopii').fill('backup password');
	await page.screenshot({ path: `${SHOTS}/16-restore.png`, fullPage: true });
	await page.getByRole('button', { name: 'Wczytaj' }).click();

	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();
	await expect(page.getByText('Wpis z kopii')).toBeVisible();
});

test('settings: export, merge import, autolock', async ({ page }) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	await expect(page.getByRole('heading', { name: 'Ustawienia' })).toBeVisible();

	await page.getByLabel('Po 5 minutach bez aktywności').check();
	await expect(page.getByText('Zapisano.')).toBeVisible();

	await page.getByRole('button', { name: 'Zapisz kopię' }).click();
	await expect(page.getByText('Kopia zapisana.')).toBeVisible();

	await page.getByRole('button', { name: 'Wczytaj kopię' }).click();
	await page.getByLabel('kluczem odzyskiwania').check();
	await page
		.getByLabel('Klucz odzyskiwania')
		.fill(Array.from({ length: 24 }, () => 'word').join(' '));
	await page.getByLabel('Zastąp: dziennik będzie dokładnie taki jak kopia').check();
	await expect(page.getByText('Wpisy, których nie ma w kopii, zostaną usunięte.')).toBeVisible();
	await page.getByLabel('Połącz: dodaj brakujące, zachowaj nowsze wersje').check();
	await page.screenshot({ path: `${SHOTS}/17-settings-import.png`, fullPage: true });
	await page.getByRole('button', { name: 'Wczytaj', exact: true }).click();
	await expect(page.getByText('Dodano 1, zaktualizowano 0, bez zmian 0.')).toBeVisible();
});

test('change password, then delete all data', async ({ page }) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();

	await page.getByLabel('Obecne hasło').fill(PASSWORD);
	await page.getByLabel('Nowe hasło').fill('a brand new password');
	await page.getByLabel('Powtórz hasło').fill('a brand new password');
	await page.getByRole('button', { name: 'Zmień hasło' }).click();
	await expect(page.getByText('Hasło zmienione.')).toBeVisible();
	await expect(page.getByText('Starsze kopie nadal otworzysz poprzednim hasłem.')).toBeVisible();

	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await page.getByLabel('Hasło', { exact: true }).fill('a brand new password');
	await page.getByRole('button', { name: 'Otwórz' }).click();
	await expect(page.getByRole('heading', { name: 'Twój dziennik' })).toBeVisible();

	await page.getByRole('link', { name: 'Ustawienia' }).click();
	await page.getByLabel('Wpisz hasło, żeby potwierdzić').fill('a brand new password');
	await page.getByRole('button', { name: 'Usuń wszystko' }).click();
	await expect(page.getByText('Na pewno usunąć wszystkie dane?')).toBeVisible();
	await page
		.getByRole('group', { name: 'Na pewno usunąć' })
		.getByRole('button', { name: 'Usuń wszystko' })
		.click();
	await expect(page.getByRole('heading', { name: 'Witaj w Mossbook' })).toBeVisible();
});

test('about: version, offline age hint, check for updates opens the browser', async ({ page }) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	await expect(page.getByText(/^Wersja 0\.1\.0 · zbudowana/)).toBeVisible();
	await expect(page.getByText('Ta wersja ma już ponad 3 miesiące.')).toBeVisible();
	await page.getByRole('button', { name: 'Sprawdź aktualizacje' }).click();
	const calls = await page.evaluate(
		() => (window as unknown as { __fake: { calls: string[] } }).__fake.calls
	);
	expect(calls).toContain('open_releases');
});

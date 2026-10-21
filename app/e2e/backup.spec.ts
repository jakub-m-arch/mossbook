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
		.getByLabel('Klucz odzyskiwania', { exact: true })
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

test('backups: last saved copy, automatic copies, recovery key check, JSON export', async ({
	page
}) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();

	await expect(page.getByText('Nie zapisano jeszcze żadnej własnej kopii.')).toBeVisible();
	await expect(page.getByText(/^Nie ma jeszcze automatycznej kopii/)).toBeVisible();
	await page.getByRole('button', { name: 'Zapisz kopię' }).click();
	await expect(page.getByTestId('last-export')).toHaveText(
		/^Ostatnia kopia zapisana przez ciebie: /
	);

	// Locking makes the automatic copy.
	await page.getByRole('button', { name: 'Zablokuj' }).click();
	await page.getByLabel('Hasło', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Otwórz' }).click();
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	await expect(page.getByTestId('auto-latest')).toHaveText(/^Ostatnia automatyczna kopia: /);
	await page.getByRole('button', { name: 'Pokaż w folderze' }).click();
	await page.getByRole('checkbox', { name: /^Rób kopię przy blokowaniu/ }).uncheck();
	await expect(page.getByTestId('auto-latest')).toBeHidden();

	// Recovery key check: wrong words, then the right ones.
	const words = Array.from({ length: 24 }, (_, i) => `word${i + 1}`);
	await page.getByLabel('Twoje 24 słowa').fill([...words].reverse().join(' '));
	await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
	await expect(
		page.getByText('Te słowa nie otwierają tego dziennika.', { exact: false })
	).toBeVisible();
	await page.getByLabel('Twoje 24 słowa').fill(words.join(' '));
	await page.getByRole('button', { name: 'Sprawdź klucz' }).click();
	await expect(page.getByText('Te słowa otwierają twój dziennik.', { exact: false })).toBeVisible();
	await expect(page.getByLabel('Twoje 24 słowa')).toHaveValue('');

	// JSON export asks first, because the file is not encrypted.
	await expect(page.getByText(/^Plik JSON nie jest zaszyfrowany/)).toBeVisible();
	await page.getByRole('button', { name: 'Zapisz plik JSON' }).click();
	await page
		.getByRole('group', { name: 'Zapisać plik bez szyfrowania?' })
		.getByRole('button', { name: 'Zapisz plik JSON' })
		.click();
	await expect(page.getByText('Plik zapisany.')).toBeVisible();
	await page.screenshot({ path: `${SHOTS}/28-settings-backups.png`, fullPage: true });
	const exports = await page.evaluate(
		() => (window as unknown as { __fake: { jsonExports: number } }).__fake.jsonExports
	);
	expect(exports).toBe(1);
});

test('about: no-warranty notice and licence texts', async ({ page }) => {
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();

	await expect(page.getByText(/Jest udostępniany bez gwarancji/)).toBeVisible();
	await expect(page.getByText(/Nie jest wyrobem medycznym/)).toBeVisible();
	await page.getByText('Licencje', { exact: true }).click();
	const licenses = page.locator('pre.licenses');
	await expect(licenses).toContainText('GNU GENERAL PUBLIC LICENSE');
	await expect(licenses).toContainText(/chacha20poly1305 \d/);
	await page.screenshot({ path: `${SHOTS}/29-settings-about.png`, fullPage: true });
});

import { createVault, expect, test } from './helpers';

test('sync: choose a folder, join the journal of another device, turn it off', async ({ page }) => {
	await page.addInitScript(() => window.localStorage.setItem('fake-sync-other', 'yes'));
	await createVault(page);
	await page.getByRole('link', { name: 'Ustawienia' }).click();
	const sync = page.getByRole('region', { name: 'Synchronizacja' });
	await expect(sync.getByText('Mossbook nie łączy się z siecią')).toBeVisible();
	await sync.getByRole('button', { name: 'Wybierz folder' }).click();
	await expect(sync.getByText('Folder: /home/synthetic/Sync')).toBeVisible();
	await expect(sync.getByText('To urządzenie')).toBeVisible();

	// Joining needs that journal's password and says what it changes.
	const join = sync.getByRole('group', {
		name: 'W tym folderze jest dziennik z innego urządzenia'
	});
	await expect(join.getByText(/przestaną działać/).first()).toBeVisible();
	await join.getByLabel('Hasło dziennika z tamtego urządzenia').fill('a wrong password');
	await join.getByRole('button', { name: 'Dołącz do tego dziennika' }).click();
	await join
		.getByRole('dialog', { name: /Dołączyć\?/ })
		.getByRole('button', { name: 'Dołącz do tego dziennika' })
		.click();
	await expect(sync.getByText('To hasło nie pasuje.', { exact: false })).toBeVisible();

	await join.getByLabel('Hasło dziennika z tamtego urządzenia').fill('phone password');
	await join.getByRole('button', { name: 'Dołącz do tego dziennika' }).click();
	await join
		.getByRole('dialog', { name: /Dołączyć\?/ })
		.getByRole('button', { name: 'Dołącz do tego dziennika' })
		.click();
	await expect(sync.getByText('Gotowe. Ten dziennik otwierasz teraz hasłem')).toBeVisible();
	await expect(join).toHaveCount(0);

	await sync.getByRole('button', { name: 'Synchronizuj teraz' }).click();
	await expect(sync.getByText('Zsynchronizowano.')).toBeVisible();
	await sync.getByRole('button', { name: 'Wyłącz synchronizację na tym urządzeniu' }).click();
	await sync
		.getByRole('dialog', { name: /Wyłączyć synchronizację\?/ })
		.getByRole('button', { name: 'Wyłącz synchronizację na tym urządzeniu' })
		.click();
	await expect(sync.getByRole('button', { name: 'Wybierz folder' })).toBeVisible();

	await page.getByRole('link', { name: 'Dziennik' }).click();
	await expect(page.getByText('from the phone')).toBeVisible();
});

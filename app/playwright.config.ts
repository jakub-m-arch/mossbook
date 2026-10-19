import { defineConfig, devices } from '@playwright/test';

// UI scenarios in Chromium against a fake Tauri backend (e2e/fake-tauri.js).
// The real backend is covered by the Rust tests.
export default defineConfig({
	testDir: 'e2e',
	fullyParallel: true,
	reporter: 'list',
	use: {
		baseURL: 'http://localhost:4173',
		locale: 'pl-PL',
		...devices['Pixel 7']
	},
	webServer: {
		command: 'npm run build && npm run preview -- --port 4173 --strictPort',
		url: 'http://localhost:4173',
		reuseExistingServer: !process.env.CI
	}
});

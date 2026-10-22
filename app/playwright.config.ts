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
		...devices['Pixel 7'],
		// Voice notes (E-18): Chromium's synthetic microphone (a beep), no prompt.
		permissions: ['microphone'],
		launchOptions: {
			args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream']
		}
	},
	webServer: {
		command: 'npm run build && npm run preview -- --port 4173 --strictPort',
		url: 'http://localhost:4173',
		reuseExistingServer: !process.env.CI
	}
});

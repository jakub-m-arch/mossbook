import { defineConfig, devices } from '@playwright/test';

// Screenshots for the website (site/img), not a test suite. Same fake backend
// and build as the e2e tests.
export default defineConfig({
	testDir: 'site-shots',
	reporter: 'list',
	use: {
		baseURL: 'http://localhost:4173',
		...devices['Pixel 7'],
		deviceScaleFactor: 2
	},
	webServer: {
		command: 'npm run build && npm run preview -- --port 4173 --strictPort',
		url: 'http://localhost:4173',
		reuseExistingServer: true
	}
});

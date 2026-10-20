// Screenshots for the website (site/), taken against the fake backend with
// synthetic entries. Run: `npx playwright test -c playwright.site.config.ts`.
import { readFileSync } from 'node:fs';

import { expect, test, type Page } from '@playwright/test';

type Lang = 'pl' | 'en';
const OUT = '../site/img';

/** Small deterministic generator, so the screenshots do not change between runs. */
function random(seed: number) {
	return () => {
		seed = (seed * 1664525 + 1013904223) % 2 ** 32;
		return seed / 2 ** 32;
	};
}

const TEXT = {
	pl: {
		tags: ['praca', 'rodzina', 'ruch', 'terapia'],
		strategies: ['spacer', 'oddech 4-7-8', 'telefon do siostry'],
		triggers: ['tłum w tramwaju', 'mail od szefa', 'głośna kłótnia za ścianą'],
		events: [
			['Rozmowa z Anią przy kawie', 'kawiarnia', 'Spokojniej niż się spodziewałam.'],
			['Spacer nad rzeką po pracy', 'bulwary', 'Głowa trochę odpoczęła.'],
			['Pierwszy trening od miesiąca', 'basen', 'Zmęczenie, ale dobre.']
		],
		notes: [
			'Zasypiam łatwiej, kiedy odkładam telefon przed dziesiątą.',
			'Dzień bez planu. Trochę czytania i długi obiad.',
			'Na sesji rozmawialiśmy o tym, jak odpoczywać bez poczucia winy.',
			'Rano mgła nad polami, wieczorem słońce.'
		]
	},
	en: {
		tags: ['work', 'family', 'movement', 'therapy'],
		strategies: ['a walk', '4-7-8 breathing', 'calling my sister'],
		triggers: ['crowded tram', 'email from my manager', 'loud argument next door'],
		events: [
			['Coffee with Anna', 'café', 'Calmer than I expected.'],
			['Walk by the river after work', 'riverside', 'My head got a little rest.'],
			['First swim in a month', 'pool', 'Tired, the good kind.']
		],
		notes: [
			'I fall asleep more easily when the phone goes away before ten.',
			'A day with no plans. Some reading and a long lunch.',
			'In the session we talked about resting without feeling guilty.',
			'Fog over the fields in the morning, sun in the evening.'
		]
	}
};

const COLORS = ['#4f7fa6', '#c8902e', '#5f7a5c', '#8a6bb0'];

/** About ten weeks of a plausible, entirely made-up journal. */
function journal(lang: Lang) {
	const text = TEXT[lang];
	const rnd = random(7);
	const tags = text.tags.map((name, i) => ({ id: `tag-${i}`, name, color: COLORS[i] }));
	const entries: unknown[] = [];
	const now = new Date();
	now.setMinutes(0, 0, 0);
	let n = 0;
	const add = (at: Date, fields: Record<string, unknown>) => {
		const iso = at.toISOString();
		entries.push({
			id: `site-${n++}`,
			schema: 1,
			created_at: iso,
			updated_at: iso,
			occurred_at: iso,
			mood: null,
			intensity: null,
			tags: [],
			note: '',
			incomplete: false,
			sensitive: false,
			...fields
		});
	};
	const at = (daysAgo: number, hour: number) => {
		const d = new Date(now);
		d.setDate(d.getDate() - daysAgo);
		d.setHours(hour, Math.floor(rnd() * 4) * 15, 0, 0);
		return d;
	};
	for (let day = 70; day >= 0; day--) {
		// A slow improvement with ordinary ups and downs.
		const base = 2.4 + (1.6 * (70 - day)) / 70 + Math.sin(day / 3) * 0.6;
		const mood = Math.max(1, Math.min(5, Math.round(base + (rnd() - 0.5) * 1.6)));
		{
			const emotions =
				mood >= 4 ? ['calm', 'joy'] : mood === 3 ? ['calm'] : ['anxiety', 'overwhelm'];
			add(at(day, rnd() < 0.5 ? 9 : 21), {
				kind: { mood: { energy: Math.max(1, mood - 1 + Math.round(rnd())), emotions } },
				mood,
				tags: rnd() < 0.4 ? [tags[Math.floor(rnd() * 3)].id] : [],
				note: day === 0 ? text.notes[0] : ''
			});
		}
		if (rnd() < 0.8) {
			// The night before this day, so nothing ends in the future.
			const bed = at(day + 1, 23);
			const wake = new Date(bed.getTime() + (6.5 + rnd() * 2) * 3600_000);
			add(wake, {
				kind: {
					sleep: {
						bedtime: bed.toISOString(),
						wake: wake.toISOString(),
						quality: Math.max(1, Math.min(5, mood + Math.round(rnd() - 0.5))),
						awakenings: Math.floor(rnd() * 3),
						nightmare: false
					}
				}
			});
		}
		if (day % 6 === 2) {
			const i = Math.floor(rnd() * 3);
			const before = 6 + Math.floor(rnd() * 3);
			add(at(day, 8 + Math.floor(rnd() * 10)), {
				kind: {
					fear: {
						trigger: text.triggers[i],
						symptoms: ['racing_heart', 'muscle_tension'],
						avoidance: false,
						strategy: text.strategies[day % 3],
						intensity_after: before - 3 - Math.floor(rnd() * 2)
					}
				},
				intensity: before,
				tags: [tags[0].id]
			});
		}
		if (day % 9 === 1) {
			const [what, place, helped] = text.events[day % 3];
			add(at(day, 17), {
				kind: { event: { what, place, body: '', thoughts: '', helped } },
				mood: 4,
				tags: [tags[(day % 2) + 1].id]
			});
		}
		if (day % 7 === 3) {
			add(at(day, 19), {
				kind: 'note',
				note: text.notes[((day / 7) % 4) | 0],
				tags: day % 14 === 3 ? [tags[3].id] : []
			});
		}
	}
	return { tags, entries };
}

async function open(page: Page, lang: Lang) {
	const crisis = JSON.parse(readFileSync('../core/data/crisis/pl.json', 'utf8'));
	await page.addInitScript((data) => {
		if (window.localStorage.getItem('fake-vault') === null)
			window.localStorage.setItem('fake-vault', 'unlocked');
		(window as Window & { __crisisData?: unknown }).__crisisData = data;
	}, crisis);
	await page.addInitScript({ path: 'e2e/fake-tauri.js' });
	await page.addInitScript(({ tags, entries }) => {
		const w = window as unknown as {
			__fake: { seed(e: unknown[]): void };
			__TAURI_INTERNALS__: { invoke(cmd: string, args: unknown): Promise<unknown> };
		};
		for (const tag of tags) w.__TAURI_INTERNALS__.invoke('tag_save', { tag, create: true });
		w.__fake.seed(entries);
	}, journal(lang));
}

async function shot(page: Page, name: string) {
	// Let charts and fonts settle; no animations in the captured frame.
	await page.evaluate(() => document.fonts.ready);
	await page.waitForTimeout(300);
	await page.screenshot({ path: `${OUT}/${name}.png`, animations: 'disabled' });
}

for (const lang of ['pl', 'en'] as const) {
	for (const scheme of ['light', 'dark'] as const) {
		test.describe(`${lang} ${scheme}`, () => {
			test.use({
				locale: lang === 'pl' ? 'pl-PL' : 'en-GB',
				timezoneId: 'Europe/Warsaw',
				colorScheme: scheme
			});
			const name = (screen: string) => `${lang}-${screen}${scheme === 'dark' ? '-dark' : ''}`;

			test(`screens ${lang} ${scheme}`, async ({ page }) => {
				await open(page, lang);
				await page.goto('/timeline');
				await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
				await shot(page, name('timeline'));

				await page.goto('/entry/new?kind=mood');
				await page.getByRole('heading', { level: 1 }).waitFor();
				const filled = {
					pl: {
						mood: 'dobrze',
						emotions: ['spokój', 'ulga'],
						note: 'Długi spacer po pracy. Wieczór bez ekranów.'
					},
					en: {
						mood: 'good',
						emotions: ['calm', 'relief'],
						note: 'A long walk after work. An evening without screens.'
					}
				}[lang];
				await page.getByRole('radio', { name: filled.mood, exact: true }).check({ force: true });
				await page.getByRole('radio', { name: '4', exact: true }).check({ force: true });
				for (const e of filled.emotions)
					await page.getByRole('button', { name: e, exact: true }).click();
				await page
					.getByLabel(lang === 'pl' ? 'Notatka' : 'Note', { exact: true })
					.fill(filled.note);
				await page.locator('body').click({ position: { x: 5, y: 5 } });
				await page.evaluate(() => window.scrollTo(0, 0));
				await shot(page, name('entry'));

				await page.goto('/patterns');
				await page.getByRole('heading', { level: 1 }).waitFor();
				await page.waitForTimeout(500);
				await shot(page, name('patterns'));

				await page.goto('/timeline');
				await page.getByRole('heading', { level: 1 }).waitFor();
				await page.getByRole('button', { name: lang === 'pl' ? 'Zablokuj' : 'Lock' }).click();
				await page.waitForURL('**/lock');
				await page.getByRole('heading', { level: 1 }).waitFor();
				await shot(page, name('lock'));
			});
		});
	}
}

// Link previews (Open Graph), 1200×630, built from the site's own fonts and a screenshot.
const OG = {
	pl: {
		file: 'og',
		title: 'Dziennik, który zostaje <em>u&nbsp;Ciebie.</em>',
		sub: 'Bez internetu, bez konta. Wszystko zaszyfrowane na Twoim urządzeniu.'
	},
	en: {
		file: 'og-en',
		title: 'A journal that stays <em>with&nbsp;you.</em>',
		sub: 'Offline, no account. Everything encrypted on your device.'
	}
};

for (const lang of ['pl', 'en'] as const) {
	test(`link preview ${lang}`, async ({ page }) => {
		await page.setViewportSize({ width: 1200, height: 630 });
		await page.route('https://og.local/**', (route) => {
			const path = new URL(route.request().url()).pathname.slice(1);
			return path === ''
				? route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>og</title>' })
				: route.fulfill({ path: `../site/${path}` });
		});
		const { file, title, sub } = OG[lang];
		await page.goto('https://og.local/');
		await page.setContent(`<!doctype html><html><head><style>
			@font-face { font-family: N; src: url(https://og.local/fonts/newsreader-latin-opsz-normal.woff2); }
			@font-face { font-family: N; src: url(https://og.local/fonts/newsreader-latin-ext-opsz-normal.woff2); unicode-range: U+0100-024F; }
			@font-face { font-family: N; font-style: italic; src: url(https://og.local/fonts/newsreader-latin-opsz-italic.woff2); }
			@font-face { font-family: N; font-style: italic; src: url(https://og.local/fonts/newsreader-latin-ext-opsz-italic.woff2); unicode-range: U+0100-024F; }
			@font-face { font-family: I; src: url(https://og.local/fonts/inter-latin-wght-normal.woff2); }
			@font-face { font-family: I; src: url(https://og.local/fonts/inter-latin-ext-wght-normal.woff2); unicode-range: U+0100-024F; }
			body { margin: 0; width: 1200px; height: 630px; background: #f5f3ed; color: #1c201b; font-family: I; overflow: hidden; position: relative; }
			.brand { position: absolute; left: 80px; top: 72px; display: flex; gap: 14px; align-items: center; font-family: N; font-size: 34px; }
			.brand img { width: 44px; height: 44px; }
			h1 { position: absolute; left: 80px; top: 190px; width: 640px; margin: 0; font-family: N; font-weight: 450; font-size: 82px; line-height: 1.02; letter-spacing: -0.015em; }
			h1 em { color: #3f6b4f; }
			p { position: absolute; left: 80px; top: 470px; width: 600px; margin: 0; font-size: 27px; line-height: 1.4; color: #4a5047; }
			.phone { position: absolute; right: 90px; top: 70px; width: 330px; padding: 9px; background: #1f231e; border-radius: 46px; box-shadow: 0 30px 60px -20px rgb(28 32 27 / 0.35); }
			.phone img { display: block; width: 100%; border-radius: 37px; }
		</style></head><body>
			<div class="brand"><img src="https://og.local/favicon.svg" alt="">Mossbook</div>
			<h1>${title}</h1><p>${sub}</p>
			<div class="phone"><img src="https://og.local/img/${lang}-timeline.png" alt=""></div>
		</body></html>`);
		await page.evaluate(() => document.fonts.ready);
		await page.waitForTimeout(300);
		await page.screenshot({ path: `${OUT}/${file}.png` });
	});
}

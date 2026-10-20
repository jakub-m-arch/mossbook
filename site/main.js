// Small progressive enhancements; the page works without them.
// Nothing here is sent anywhere. The only thing stored is the language choice
// (localStorage, in this browser).
(() => {
	const doc = document.documentElement;
	const lang = doc.lang;

	// The Polish page is the root. Visitors who chose English, or whose browser
	// does not prefer Polish and who have not chosen yet, go to the English page.
	try {
		const chosen = localStorage.getItem('lang');
		const prefersPolish = navigator.languages.some((l) => l.startsWith('pl'));
		if (lang === 'pl' && (chosen === 'en' || (!chosen && !prefersPolish))) {
			location.replace('en/' + location.hash);
			return;
		}
	} catch {
		// Storage blocked: stay on this page.
	}
	for (const link of document.querySelectorAll('[data-lang]')) {
		link.addEventListener('click', () => {
			try {
				localStorage.setItem('lang', link.dataset.lang);
			} catch {
				// Not essential.
			}
		});
	}

	// Point the main button at the visitor's system, and mark its row.
	const ua = navigator.userAgent;
	const os = /Mac/.test(ua) && !/iPhone|iPad/.test(ua)
		? 'mac'
		: /Windows/.test(ua)
			? 'windows'
			: /Linux/.test(ua) && !/Android/.test(ua)
				? 'linux'
				: null;
	const primary = document.querySelector('[data-primary]');
	if (os && primary) {
		const row = document.getElementById(`get-${os}`);
		row?.classList.add('detected');
		const label = primary.dataset[os];
		if (label) primary.querySelector('span').textContent = label;
		primary.href = `#get-${os}`;
	}

	// Copy the Homebrew command.
	for (const button of document.querySelectorAll('[data-copy]')) {
		button.hidden = !navigator.clipboard;
		button.addEventListener('click', async () => {
			const text = document.getElementById(button.dataset.copy).textContent;
			await navigator.clipboard.writeText(text);
			const original = button.textContent;
			button.textContent = button.dataset.done;
			setTimeout(() => (button.textContent = original), 1800);
		});
	}
})();

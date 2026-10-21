# Website

The static site published at <https://jakub-m-arch.github.io/mossbook/> by
`.github/workflows/pages.yml`: `index.html` (Polish, the root) and `en/index.html`,
one stylesheet, one small script and local fonts. Nothing is loaded from other
servers; the Content-Security-Policy in each page enforces it.

- **Preview:** `site/render.sh 0.1.2 /tmp/mossbook-site && python3 -m http.server -d /tmp/mossbook-site`
  (`{{version}}` in the pages becomes the release version; the workflow uses the latest
  published release). Add `android` as a third argument to render the Android download
  and texts, kept between `<!--android-->` and `<!--/android-->`; without it the text
  between `<!--pre-android-->` and `<!--/pre-android-->` is used. The workflow passes
  `android` when the release has an APK.
- **Screenshots** in `img/` come from the app with synthetic entries, light and dark:
  `cd app && npx playwright test -c playwright.site.config.ts`. Run it again after UI
  changes.
- **Fonts:** Newsreader and Inter (SIL Open Font License 1.1, see `fonts/`).

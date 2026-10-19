# Mossbook: plan implementacji

Plan wykonawczy do `SPEC.md` (§12). Każdy etap kończy się zielonym CI i osobnym PR-em.
Legenda: `[ ]` do zrobienia, `[x]` zrobione.

---

## 1. Przygotowanie środowiska (jednorazowo)

Stan na 2026-10-18 (macOS): jest Node 26 i npm, są Xcode Command Line Tools. Zainstalowano Rust 1.99.1, tauri-cli 2.12.3, cargo-deny 0.20.2, cargo-audit 0.22.2. **Brakuje pełnego Xcode, JDK i Android SDK.**

- [x] Rust: `rustup` (stable) + `rustfmt`, `clippy`
- [x] `cargo install tauri-cli cargo-deny cargo-audit --locked`
- [ ] (etap 11) JDK 17, Android Studio / SDK + NDK, `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`
- [ ] (v1, iOS) pełny Xcode
- [ ] `gh` CLI (opcjonalnie, do PR-ów)

---

## 2. Etapy

### Etap 1: Szkielet
- [x] Workspace Cargo: `core` (`mossbook-core`), `app/src-tauri` (`mossbook`); `rust-toolchain.toml` (1.99.1), resolver 3, edition 2024, profil `release` (`lto`, `codegen-units = 1`, `panic = "abort"`, `strip`)
- [x] `app/`: SvelteKit 3 + `adapter-static` (SPA, `fallback: index.html`, `ssr = false`), Svelte 5 (runes wymuszone), TS, Vite 8, Vitest, ESLint, Prettier
- [x] Tauri 2.12: CSP z SPEC §8.4 (+ `devCsp` z websocketem HMR), `freezePrototype`, capabilities puste, usunięty `tauri-plugin-log`; identyfikator `io.github.jakubmarch.mossbook`
- [x] `deny.toml`: licencje zgodne z GPL-3.0, bany crate'ów sieciowych i `tauri-plugin-log`, tylko crates.io
- [x] Lint „brak logowania”: clippy `print_stdout`/`print_stderr`/`dbg_macro` + `disallowed-macros` dla `log::*`/`tracing::*`; ESLint `no-console`
- [x] CI: fmt, clippy, test, deny, audit, `npm run check`/`lint`/`test`, build desktop ubuntu/windows/macos (bez podpisywania)
- [x] Pusta aplikacja startuje (lokalnie: macOS, `cargo tauri build --debug --no-bundle`)
- [ ] CI zielone na GitHubie (po pierwszym pushu)

**Ustalenia z etapu 1:**
- Tauri na Androidzie/iOS zawsze zależy od `reqwest` (+`hyper`), ale używa go tylko pod `cfg(all(dev, mobile))` do proxy dev serwera. `deny.toml` dopuszcza `reqwest` wyłącznie jako zależność `tauri`. Etap 11: sprawdzić, że release `.so` nie zawiera symboli `reqwest`/`hyper`, oprócz testu manifestu.
- `cargo audit` zgłasza ostrzeżenia (unmaintained/unsound) dla gtk-rs 0.18 na Linuksie, przychodzące z Tauri. Nie blokują CI; `RUSTSEC-2024-0370` jest wyciszony w `deny.toml` z uzasadnieniem.
- Do rozważenia: przypięcie akcji GitHub do SHA (łańcuch dostaw).

### Etap 2: Core – kryptografia (`core/src/crypto/`)
- [x] `kdf.rs`: Argon2id, `KdfParams { m_kib, t, p }`, 13-bajtowe kodowanie do nagłówka, walidacja górnych limitów, `calibrate(CALIBRATION_TARGET)` (podnosi tylko `t`), normalizacja hasła NFC
- [x] `aead.rs`: XChaCha20-Poly1305, `nonce(24) || ct || tag(16)`, AAD rekordu z SPEC §6.2, `RecordType` z kodami kolumny
- [x] `wrap.rs`: wrap/unwrap DEK, AAD `"mossbook/wrap/v1" || vault_id`
- [x] `recovery.rs`: 256 bit ↔ 24 słowa BIP-39, HKDF-SHA256 → KEK_rec, normalizacja wejścia, `confirm()` dla V-1
- [x] `Dek`, `Kek`, `RecoveryKey` w `SecretBox`, bez `Clone`, `Debug` z redakcją; bufory pośrednie zerowane
- [x] Testy (40): round-trip, złe hasło/klucz, każdy podmieniony bajt, zamiana rekordów, inny typ/skarbiec, wrogie parametry KDF, cała hierarchia + zmiana hasła, wektor BIP-39, proptest
- [x] Stałe wektory potwierdzone niezależnie (libsodium, argon2-cffi): `tools/verify-vectors.py`
- [x] `docs/crypto.md`

### Etap 3: Core – skarbiec i magazyn (`vault/`, `store/`)
- [x] `store/`: `rusqlite` (`bundled`), schemat z SPEC §7 (`STRICT`), `WAL`, `secure_delete = ON`, migracje przez `PRAGMA user_version`, odmowa otwarcia bazy z nowszej wersji
- [x] `VaultHeader` w `meta`, `UnlockedVault` (DEK + rekordy w pamięci, zerowane przy `Drop`); rekordy jako bajty, typy domenowe w etapie 4
- [x] `PendingVault` (D-3): skarbiec w pamięci, zapis dopiero po potwierdzeniu 3 słów, przez `vault.db.creating` + rename
- [x] `unlock`, `unlock_with_recovery` (+ nowe hasło), `lock` (checkpoint WAL), `change_password` (rewrap, nowa sól), `destroy` (oddaje skarbiec przy złym haśle)
- [x] `UnlockThrottle`: 1, 2, 4… 60 s, tylko w pamięci (D-5)
- [x] Uszkodzone rekordy są pomijane i liczone, a nie blokują całego skarbca
- [x] Testy integracyjne (14): cykl życia, złe hasło/fraza, odzyskiwanie, zmiana hasła, usuwanie, brak jawnej treści na dysku, podmiana/zamiana rekordów, podmiana nagłówka, fixture formatu v1
- [x] `docs/crypto.md`: sekcja *Vault file*

### Etap 4: Core – domena i zapytania
- [x] `domain/`: `Entry` (+`EntryKind` ×6), `Tag`, `SafetyPlan`, `Settings`, `Emotion`/`Symptom` z `Custom(String)`; walidacja zakresów; UUIDv7; CBOR; znaczniki czasu RFC 3339 z offsetem; `ZeroizeOnDrop` i `Debug` z redakcją
- [x] `Journal`: CRUD wpisów (zachowuje `created_at`, odrzuca nieznane tagi), szybki wpis, tagi (tworzenie, zmiana nazwy/koloru, usuwanie i łączenie w jednej transakcji), plan i ustawienia jako singletony
- [x] `UnlockedVault` oddaje odszyfrowane rekordy raz (`take_loaded`), więc treść jest w pamięci tylko jako obiekty domenowe; zapis wsadowy `write_batch`
- [x] `query/`: filtr (typ, tagi, zakres dat, tylko do uzupełnienia), wyszukiwanie bez wielkości liter i znaków diakrytycznych (także `ł`, tagi), `mood_by_day` dla A-1
- [x] Testy: jednostkowe domeny i zapytań, integracyjne dziennika (8), proptest: dowolne `Entry` → zapis → blokada → odczyt
- [x] `docs/crypto.md`: sekcja *Record contents*

### Etap 5: Tauri – komendy i stan
- [x] `Session` (czysty Rust, testowalny): stany `no_vault` / `creating` / `locked` / `unlocked`, opóźnienie po błędnych próbach, zmiana hasła, usuwanie (przy złym haśle dziennik zostaje nietknięty)
- [x] 26 komend z SPEC §8.3 (bez kopii i PDF), wszystkie `async` (Argon2 nie blokuje UI); hasła od razu w `Zeroizing`; `AppError { code, message, retry_after_ms }`
- [x] Auto-blokada w Ruście: `Suspended` (mobile) i minimalizacja (desktop) → blokada przy polityce „natychmiast”; utrata fokusu → tylko zdarzenie `privacy-curtain` (D-11); timer bezczynności co 5 s, frontend tylko go resetuje (`activity_ping`); okna systemowe wstrzymują blokadę w tle, ale nie timer (D-6)
- [x] Czas lokalny: UI podaje offset (`clock_sync`), bo `now_local()` jest niewiarygodne w procesie wielowątkowym
- [x] Uprawnienia: `AppManifest` generuje `allow-<komenda>`, capability nadaje dokładnie te + `core:event:allow-(un)listen`; test pilnuje zgodności listy komend, capability i `generate_handler!`
- [x] `app/src/lib/api/`: typy TS i wrappery `invoke()` (D-7: ręcznie + test kształtów JSON w Ruście + test Vitest porównujący wrappery z listą komend w Ruście)
- [x] Aplikacja startuje lokalnie (macOS)

### Etap 6: UI – zakładanie skarbca i blokada
- [ ] `setup`: hasło + wskaźnik siły (lokalnie, np. `zxcvbn-ts` bez słowników z sieci), wyświetlenie 24 słów, potwierdzenie 3 losowych, komunikat z kryteriów V-1
- [ ] `lock`: hasło, odzyskiwanie kluczem, przyciski „Plan na trudny moment / telefony”
- [ ] Przycisk blokady zawsze widoczny (layout)

### Etap 7: UI – wpisy
- [ ] Szybki wpis (≤ 2 stuknięcia, flaga `incomplete`)
- [ ] Formularze E-1, E-2, E-3, E-4, E-6; zapis po wypełnieniu jednego pola
- [ ] Oś czasu z filtrami i wyszukiwaniem; edycja, usuwanie z potwierdzeniem; tagi
- [ ] Czyszczenie schowka po 60 s (tylko jeśli schowek nadal zawiera nasz tekst)

### Etap 8: Plan na trudny moment
- [ ] Edytor planu Stanley–Brown (P-1), dostęp z ekranu głównego (P-2)
- [ ] `core/data/crisis/pl.json` (+ `source`, `verified_at`), widok bez odblokowania (P-3)
- [ ] Ćwiczenia 4-7-8, pudełkowy, 5-4-3-2-1 z obsługą `prefers-reduced-motion` (P-4)

### Etap 9: Kopia `.mossbook`
- [ ] `backup/format.rs` + `docs/backup-format.md` (magic, wersja, vault_id, KDF, opakowane DEK, rekordy)
- [ ] Eksport przez systemowe okno zapisu; import: scalanie / zastąpienie (D-4)
- [ ] Testy: eksport → import do pustego skarbca → identyczne dane; uszkodzony plik; zła wersja; stały plik testowy v1

### Etap 10: Wykres i i18n
- [ ] A-1 z `uPlot` (lokalnie), tydzień/miesiąc/3 mies.
- [ ] i18n pl/en/uk (np. `svelte-i18n` lub własny lekki moduł), klucze dla emocji/objawów

### Etap 11: Android
- [ ] `cargo tauri android init`, commit `gen/android`
- [ ] Manifest release bez `INTERNET` (`tools:node="remove"`), `allowBackup=false`, `dataExtractionRules`, `FLAG_SECURE` w `MainActivity`
- [ ] Test CI na zbudowanym manifeście (`aapt dump permissions`)
- [ ] Sprawdzić, czy zmiany przetrwają ponowny `init` (SPEC §8.4)

### Etap 12: Wydanie 0.1.0
- [ ] `SECURITY.md`, `docs/threat-model.md`, README, `CONTRIBUTING.md` (DCO, zasady z CLAUDE.md)
- [ ] Release z sumami kontrolnymi; weryfikacja numerów kryzysowych

### Etap 13 (v1)
Biometria, szybki wpis bez odblokowania (sealed box), IRT, PDF, A-2–A-4, przypomnienia, iOS, kalendarz, „treść trudna”.

---

## 3. Decyzje i luki w specyfikacji

Status: **przyjęte** (2026-10-18) albo **otwarte**. Przyjęte decyzje wiążą implementację tak samo jak SPEC.md.

| ID | Status | Problem | Decyzja / propozycja |
|---|---|---|---|
| D-1 | przyjęte | **Licencja** | `GPL-3.0-or-later` + wyjątek z sekcji 7 na dystrybucję przez App Store / Google Play (`LICENSE-EXCEPTION.md`). Copyleft blokuje zamknięte forki z telemetrią; wyjątek usuwa znany konflikt GPL z regulaminem App Store. Wkłady przez DCO (`Signed-off-by`). Zależności muszą mieć licencje zgodne z GPL-3.0 (pilnuje `cargo deny`) |
| D-2 | przyjęte | **Ustawienia przed odblokowaniem**: ekran blokady potrzebuje języka i kraju numerów kryzysowych, a `Settings` są zaszyfrowane (zasada 2) | Przed odblokowaniem: język i kraj z ustawień systemu. Ewentualnie jawne `ui_locale` w `meta`, udokumentowane jako akceptowany wyciek |
| D-3 | przyjęte | **Moment utworzenia skarbca**: jeśli aplikacja zostanie zamknięta między `vault_create` a `vault_confirm_recovery`, powstaje skarbiec bez potwierdzonego klucza | Skarbiec powstaje w pamięci, a na dysk trafia dopiero po potwierdzeniu słów. Komenda `vault_create` zwraca frazę, `vault_confirm_recovery` zapisuje plik |
| D-4 | przyjęte | **Import z innego skarbca**: AAD zawiera `vault_id`, więc rekordów z kopii innego skarbca nie da się po prostu skopiować | Import zawsze odszyfrowuje kluczem z kopii i szyfruje ponownie DEK bieżącego skarbca. Konflikt ID przy scalaniu: wygrywa nowsze `updated_at` |
| D-5 | otwarte | **Licznik błędnych prób (V-8)**: trwały licznik na dysku to jawna metadana, a atakujący offline i tak go omija | Licznik tylko w pamięci procesu. Prawdziwą ochroną jest Argon2id, co opisujemy w threat-model |
| D-6 | przyjęte | **Auto-blokada a okna systemowe**: okno zapisu kopii albo picker na Androidzie powoduje utratę fokusu lub przejście w tło, co zablokowałoby skarbiec w trakcie eksportu | Flaga „operacja systemowa w toku” w Ruście wstrzymuje blokadę „natychmiast”, ale nie timer bezczynności |
| D-7 | przyjęte | **Typy TS**: ręczne utrzymywanie typów to ryzyko rozjazdu | `specta` + `tauri-specta` (generowanie `bindings.ts`), jeśli wersja zgodna z Tauri 2 jest stabilna; inaczej ręcznie + test snapshot |
| D-8 | otwarte | **Kolumna `type` jawna** ujawnia liczbę wpisów w odróżnieniu od tagów | Akceptujemy i dopisujemy do listy wycieków w SPEC §2 (albo jeden typ „record” z typem tylko w AAD/treści) |
| D-9 | otwarte | **Stara kopia po zmianie hasła** otwiera się starym hasłem (zawiera DEK opakowany starym KEK) | Udokumentować w UI zmiany hasła i w `docs/backup-format.md` |
| D-10 | otwarte | **Router frontendu**: SPEC wymienia `routes/` | SvelteKit + `adapter-static` (oficjalny szablon Tauri) |
| D-11 | przyjęte | **Desktop „natychmiast po przejściu w tło”**: na desktopie utrata fokusu zdarza się ciągle | Desktop: domyślnie blokada przy minimalizacji/ukryciu + timer 5 min; utrata fokusu tylko zasłania okno |

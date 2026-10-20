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
- [x] `setup`: powitanie z komunikatem z §9, hasło + powtórzenie + lokalny wskaźnik siły (bez zależności), 24 słowa z ostrzeżeniem z V-1, „zapisałem” wymagane, przepisanie 3 słów
- [x] `lock`: hasło, odliczanie opóźnienia po błędnej próbie, „Nie pamiętam hasła”; `recover`: 24 słowa + nowe hasło
- [x] Przycisk blokady zawsze widoczny po odblokowaniu (`UnlockedShell`), komunikat o nieczytelnych rekordach
- [x] Zdarzenia z Rusta: `vault-locked` → ekran blokady, `privacy-curtain` → zasłona; pingi aktywności co ≥ 20 s; `clock_sync` przy starcie i powrocie
- [x] i18n pl/en/uk (język systemu przed odblokowaniem, D-2), test zgodności kluczy i placeholderów
- [x] Style: spokojna paleta, tryb ciemny, cele ≥ 48 px, `prefers-reduced-motion`, bez czerwieni w ostrzeżeniach
- [x] E2E (Playwright, Chromium, udawany backend `e2e/fake-tauri.js`): zakładanie, blokada, złe hasło z opóźnieniem, odblokowanie, auto-blokada z Rusta, zasłona, odzyskiwanie; w CI
- [ ] Przycisk „Plan na trudny moment / telefony” na ekranie blokady (etap 8)
- [ ] Tłumaczenia uk do przejrzenia przez native speakera przed wydaniem

### Etap 7: UI – wpisy
- [x] Szybki wpis (E-10): rodzaj + ocena/intensywność = 2 stuknięcia; sen i notatka 1 stuknięcie; zapis jako „do uzupełnienia”, po czym od razu otwiera się formularz wpisu (można uzupełnić teraz albo wyjść)
- [x] Formularze E-1 (piktogramy pogody 1–5, nie emoji), E-2, E-3 (objawy + własne, unikanie, strategia, intensywność po), E-4, E-6; koszmar (E-5) tylko przy edycji istniejących (tworzenie w v1); zapis po jednym polu; pełny zapis kończy „do uzupełnienia”
- [x] Oś czasu z filtrami (rodzaj, tagi, zakres dat, tylko do uzupełnienia) i wyszukiwaniem w Ruście; edycja; usuwanie z potwierdzeniem w miejscu (bez `confirm()`)
- [x] Tagi (E-12): tworzenie, zmiana nazwy, kolor z palety, usuwanie, łączenie
- [x] Schowek: po `copy` Rust czyści schowek po 60 s, jeśli nadal zawiera skopiowany tekst (`tauri-plugin-clipboard-manager`, używany tylko z Rusta; BSL-1.0 dopuszczona w `deny.toml`)
- [x] E2E: szybki wpis, pełny formularz + edycja + usunięcie, tagi + filtr + wyszukiwanie bez diakrytyków; każdy test pada na błędzie w konsoli
- [ ] Język z ustawień po odblokowaniu: razem z ekranem ustawień (etap 10), bo domyślne `locale = "pl"` nadpisałoby język systemu

### Etap 8: Plan na trudny moment
- [x] Edytor planu Stanley–Brown (P-1), sześć sekcji, kontakty z przyciskiem połączenia; dostęp jednym przyciskiem z osi czasu (P-2)
- [x] `core/data/crisis/pl.json` (+ `sources`, `verified_at`), moduł `core::crisis` z walidacją plików w testach; numery zweryfikowane 2026-10-19 ze stroną MZ (gov.pl) i stronami operatorów
- [x] Ekran pomocy bez odblokowania (P-3): numery z przyciskiem „Zadzwoń”, kraj z regionu języka systemu (D-2), komunikat dla krajów bez danych
- [x] `dial`: tylko numery kryzysowe i zapisane kontakty z planu (po odblokowaniu); `tauri-plugin-opener` używany wyłącznie z Rusta
- [x] Ćwiczenia offline (P-4): oddech 4-7-8 i pudełkowy z animacją (bez animacji przy `prefers-reduced-motion`), 5-4-3-2-1
- [x] Przycisk „Plan na trudny moment / telefony” na ekranie blokady
- [x] E2E: pomoc z ekranu blokady (numery, połączenie, ćwiczenia), plan (zapis, połączenie tylko z zapisanym kontaktem)
- [ ] Teksty ćwiczeń do konsultacji z psychoterapeutą przed v1 (SPEC §9); numery do ponownej weryfikacji przed każdym wydaniem

### Etap 9: Kopia `.mossbook`
- [x] `core::backup`: `MOSSBOOK` + wersja + CBOR; nagłówek i zaszyfrowane rekordy 1:1 z bazy; manifest (SHA-256 rekordów, szyfrowany DEK) wykrywa usunięcie, dodanie i zmianę kolejności; limit 512 MiB
- [x] Otwieranie hasłem albo kluczem odzyskiwania z dnia kopii (D-9); przywracanie na nowym urządzeniu bez ponownego szyfrowania; import do istniejącego dziennika z ponownym szyfrowaniem (D-4): scalanie (nowsze `updated_at`, tagi po nazwie, plan i ustawienia bez zmian) albo zastąpienie, w jednej transakcji
- [x] Komendy `backup_export` / `backup_pick` / `backup_import` z systemowymi oknami (plugin `dialog` + `fs`, obsługa `content://` na Androidzie); blokada w tle wstrzymana na czas okna (D-6), mutex sesji nie jest trzymany w trakcie
- [x] Ekran ustawień: kopia (zapis, wczytanie), automatyczna blokada (V-3), zmiana hasła (V-5), usunięcie wszystkich danych (V-9); „Mam kopię zapasową” na pierwszym ekranie
- [x] `docs/backup-format.md` + niezależny czytnik `tools/read-backup.py` (libsodium, argon2-cffi, cbor2), sprawdzony na fixture
- [x] Testy: eksport → przywrócenie / import do innego skarbca = identyczne dane, scalanie, złe hasło/klucz, manipulacje (bajt, usunięcie, zamiana, obcy manifest, obcięty plik, wersja), stary hasło po zmianie, fixture `backup-v1.mossbook`; E2E: przywracanie, ustawienia, zmiana hasła, usunięcie danych

### Etap 10: Wykres i i18n
- [x] A-1: zakładka „Wzorce”, wykres samopoczucia (tydzień / miesiąc / 3 miesiące) jako własny SVG zamiast uPlot (D-12): linia 2 px, znaczniki z obwódką, siatka, krzyżyk + podpowiedź (mysz i strzałki), widok tabeli; kolory serii zwalidowane (jasny/ciemny); tylko obserwacje, bez wniosków klinicznych
- [x] Język w ustawieniach: „jak w systemie”, Polski, English, Українська; `Settings.locale` opcjonalne (stare rekordy z napisem nadal się wczytują), stosowane po odblokowaniu (D-2)
- [x] Teksty z liczbami przeformułowane tak, żeby nie wymagały odmiany liczebników
- [ ] Tłumaczenia uk do przejrzenia przez native speakera przed wydaniem

### Etap 11: Android — odłożony
Świadomie odłożony (decyzja z 2026-10-19): brak narzędzi (JDK, SDK, NDK), a dystrybucja w sklepach jest niepewna. Do zrobienia przy powrocie: `cargo tauri android init`, manifest wydania bez `INTERNET`, `allowBackup=false` + `dataExtractionRules`, `FLAG_SECURE`, test manifestu i sprawdzenie, że release `.so` nie zawiera `reqwest`/`hyper`.

### Etap 12: Wydanie 0.1.0 (desktop)
- [x] `SECURITY.md`, `docs/threat-model.md` (zagrożenie → ochrona → miejsce w kodzie, akceptowane wycieki, ograniczenia), `CONTRIBUTING.md` (DCO, zasady), `CHANGELOG.md`
- [x] Workflow `Release`: tag `v*` → instalatory Linux / Windows / macOS (arm i x86) + `SHA256SUMS` jako **szkic** wydania
- [x] `docs/release.md`: lista kontrolna (weryfikacja numerów kryzysowych, niezależne narzędzia, wersje, ręczny test)
- [x] Test spójności wersji (Cargo, `tauri.conf.json`, `package.json`)
- [x] Poprawka przed wydaniem: losowe UUIDv4 zamiast v7 (D-13)
- [ ] Włączyć prywatne zgłaszanie podatności w ustawieniach repozytorium
- [x] Ręczny test wersji release (instalacja .dmg, raport PDF, szybki wpis) i tag `v0.1.0` (2026-10-20)

### Etap 13 (v1)
- [x] **Raport PDF dla terapeuty (X-3)**: wybór okresu, rodzajów i tagów; frontend buduje model dokumentu (tłumaczenia), ten sam model jest podglądem i trafia do `core::report`, który składa A4 przez `printpdf` (łamanie tekstu po szerokościach glifów, podział na strony, stopka „nie jest zaszyfrowany” + numer strony); Noto Sans (OFL) osadzony w całości (~600 KB, podzbiór wymagałby ciężkiej funkcji `text_layout`); ostrzeżenie przed zapisem; zapis przez okno systemowe
- [x] **Koszmar / IRT (E-5)**: tworzenie (szybki wpis jednym stuknięciem), wyjaśnienie metody, treść koszmaru zwinięta przy ponownym otwarciu, nowa wersja, powtórki z datami; domyślnie „treść trudna”
- [x] **„Treść trudna” (E-16)**: przełącznik w każdym formularzu; na osi czasu tylko rodzaj i godzina, odsłonięcie stuknięciem
- [x] **Kalendarz (E-14)** we Wzorcach: miesiąc, odcień dnia wg średniego samopoczucia (jeden odcień, bez czerwieni), legenda, nawigacja po miesiącach; stuknięcie w dzień → oś czasu z filtrem tego dnia
- [x] **Statystyki A-2–A-4** (`core::query::patterns`, zakładka Wzorce → Zestawienia): od 14 wpisów w okresie; najczęstsze tagi i wyzwalacze (wolny tekst grupowany po normalizacji, ≥ 2 powtórzenia); samopoczucie a sen poprzedniej nocy (słaby/średni/dobry), pora dnia, dzień tygodnia; strategie: średni spadek intensywności (≥ 2 użycia); paski z liczbami, tylko obserwacje
- [ ] Przypomnienia (R-1–R-3): **odłożone do wersji mobilnych** (D-14)
- [x] **Szybki wpis bez odblokowania (E-15)**: para X25519 w nagłówku (`x25519_pub`, `wrapped_x25519_priv`), sealed box zgodny z libsodium (test z PyNaCl), tabela `inbox`, minimalny `InboxEntry` (rodzaj, wartość, czas), przeniesienie do dziennika przy odblokowaniu, starsze skarbce dostają klucze przy następnym odblokowaniu; przycisk na ekranie blokady
- [ ] Biometria, iOS, Android (odłożone)
| D-14 | przyjęte | **Przypomnienia na desktopie**: harmonogram jest w zaszyfrowanych ustawieniach, a przypomnienie musi przyjść przy zablokowanym dzienniku; na desktopie tylko przy działającej aplikacji | Odłożone do wersji mobilnych (systemowe planowanie powiadomień). Bez jawnego harmonogramu na dysku |
| D-15 | przyjęte | **Aktualizacje bez sieci**: auto-update łamie zasadę 1 (połączenie ujawnia IP i fakt używania aplikacji) | Ustawienia → „O aplikacji”: wersja, data buildu (`SOURCE_DATE_EPOCH` dla powtarzalności), podpowiedź po 90 dniach liczona lokalnie, przycisk otwierający stronę Releases w przeglądarce (stały URL w Ruście); README: powiadomienia GitHuba. Później: Homebrew / winget / Flathub aktualizują z zewnątrz |

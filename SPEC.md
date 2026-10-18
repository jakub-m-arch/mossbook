# Mossbook: prywatny dziennik samopoczucia

> Dokument jest specyfikacją projektu dla Claude Code: wizja, wymagania biznesowe, model zagrożeń, architektura, kryptografia i kolejność implementacji.

---

## 1. Wizja

Mossbook to otwartoźródłowa aplikacja do prowadzenia osobistego dziennika: samopoczucia, zdarzeń, lęków oraz snu i koszmarów. Jest przeznaczona dla każdego, ale projektowana z myślą o osobach z PTSD, zaburzeniami lękowymi i tych, które pracują z terapeutą.

**Obietnica dla użytkownika:** nikt poza tobą nie przeczyta twoich wpisów. Nie ma serwera, konta ani telemetrii. Dane są zaszyfrowane na twoim urządzeniu, a kod jest otwarty, więc każdy może to sprawdzić.

### Zasady nienaruszalne

1. **Zero sieci.** Aplikacja nie wykonuje żadnych połączeń sieciowych. Na Androidzie wydanie produkcyjne nie ma nawet uprawnienia `INTERNET`.
2. **Wszystko zaszyfrowane w spoczynku.** Na dysku nie ma żadnej treści wpisu, tagu, daty ani ustawienia w postaci jawnej.
3. **Zero telemetrii i analityki**, także „anonimowej”. Brak raportowania błędów na zewnątrz.
4. **Użytkownik jest właścicielem danych.** Pełny eksport, pełne usunięcie, otwarty format kopii.
5. **To nie jest wyrób medyczny.** Aplikacja nie diagnozuje, nie zaleca leczenia i nie ocenia stanu zdrowia. Jest narzędziem do zapisywania i porządkowania własnych obserwacji.
6. **Spokój zamiast zaangażowania.** Bez serii dni, punktów, odznak i powiadomień wywołujących poczucie winy.

---

## 2. Model zagrożeń

| Zagrożenie | Przykład | Ochrona |
|---|---|---|
| Domownik lub partner z dostępem do odblokowanego telefonu | Ktoś bierze telefon i otwiera aplikację | Osobne hasło lub PIN aplikacji, auto-blokada po wyjściu i po bezczynności |
| Fizyczny dostęp do plików | Kradzież, serwis, kopia dysku | Szyfrowanie całej zawartości (XChaCha20-Poly1305), klucz z hasła przez Argon2id |
| Automatyczne kopie w chmurze | iCloud, Google Backup | Wyłączenie aplikacji z kopii systemowych; nawet gdyby wyciekły, dane są zaszyfrowane |
| Podgląd ekranu | Przełącznik aplikacji, zrzuty ekranu, nagrywanie ekranu | `FLAG_SECURE` (Android), zasłona przy przejściu w tło (iOS, desktop) |
| Powiadomienia | Treść widoczna na ekranie blokady | Powiadomienia wyłącznie lokalne, z neutralną treścią („Czas na chwilę dla siebie”) |
| Schowek | Skopiowany fragment wpisu zostaje w schowku | Czyszczenie schowka po 60 s |
| Logi aplikacji | Treść wpisu trafia do logcat lub konsoli | Zakaz logowania danych użytkownika, egzekwowany w code review i lintem |
| Zapomniane hasło | Utrata dostępu | Klucz odzyskiwania generowany przy zakładaniu skarbca; jasna informacja, że bez niego dane są nie do odzyskania |
| Manipulacja plikiem | Podmiana lub uszkodzenie rekordów | Szyfrowanie uwierzytelnione (AEAD) z AAD powiązanym z identyfikatorem rekordu |

**Poza zakresem:** urządzenie z malware lub rootem, keylogger, przymus fizyczny („pokaż mi telefon”), analiza pamięci RAM w trakcie, gdy skarbiec jest odblokowany.

**Wycieki metadanych, które akceptujemy i dokumentujemy:** liczba rekordów, przybliżony rozmiar wpisów i czas ostatniej modyfikacji pliku bazy.

---

## 3. Użytkownicy

- **Anna, 34 lata, PTSD po wypadku.** Chce zapisywać, co ją wytrąciło z równowagi i jak sobie poradziła, żeby pokazać to terapeutce. Często pisze w złym stanie, więc wpis musi zająć kilka sekund.
- **Marek, 41 lat, koszmary.** Pracuje metodą IRT (Imagery Rehearsal Therapy). Potrzebuje miejsca na zapis koszmaru i jego „nowej wersji”, którą ćwiczy.
- **Ola, 27 lat, lęk uogólniony.** Chce zobaczyć, czy lęk ma związek ze snem, pracą lub porą dnia.
- **Terapeuta (pośrednio).** Dostaje od pacjenta wydruk lub PDF z wybranego okresu. Nie ma dostępu do aplikacji.

---

## 4. Funkcjonalności

Priorytety: **MVP** (pierwsze wydanie), **v1** (kolejne), **Później**.

### 4.1. Skarbiec i dostęp

| ID | Funkcja | Priorytet |
|---|---|---|
| V-1 | Założenie skarbca: hasło (min. 8 znaków, wskaźnik siły) oraz wygenerowany **klucz odzyskiwania** (24 słowa), który użytkownik musi potwierdzić, przepisując 3 losowe słowa | MVP |
| V-2 | Odblokowanie hasłem | MVP |
| V-3 | Auto-blokada: natychmiast po przejściu w tło (domyślnie) lub po 1, 5 albo 15 min, konfigurowalna | MVP |
| V-4 | Ręczna blokada jednym przyciskiem, zawsze widocznym | MVP |
| V-5 | Zmiana hasła bez ponownego szyfrowania danych (rewrap klucza, patrz §6) | MVP |
| V-6 | Odzyskanie dostępu kluczem odzyskiwania i ustawienie nowego hasła | MVP |
| V-7 | Odblokowanie biometrią (opcjonalne, wymaga wcześniejszego hasła) | v1 |
| V-8 | Opóźnienie po błędnych próbach (1 s, 2 s, 4 s… maks. 60 s); brak automatycznego kasowania danych | MVP |
| V-9 | Trwałe usunięcie skarbca z potwierdzeniem hasłem | MVP |
| V-10 | Neutralna ikona i nazwa aplikacji do wyboru (np. „Notatki”) | Później |

**Kryteria akceptacji V-1:** bez potwierdzenia klucza odzyskiwania nie da się przejść dalej. Ekran wprost mówi: „Jeśli zgubisz hasło i ten klucz, nikt (także my) nie odzyska twoich wpisów”.

### 4.2. Wpisy

Wspólna struktura: każdy wpis ma typ, czas zdarzenia (domyślnie „teraz”, edytowalny), opcjonalną ocenę samopoczucia, intensywność, tagi i notatkę tekstową.

| ID | Typ wpisu | Pola specyficzne | Priorytet |
|---|---|---|---|
| E-1 | **Samopoczucie** | ocena 1–5 (piktogramy, nie twarze z emoji), energia 1–5, opcjonalnie emocje z listy (lęk, smutek, złość, wstyd, ulga, spokój…) | MVP |
| E-2 | **Zdarzenie** | co się stało, gdzie (tekst, **bez GPS**), reakcja ciała, myśli, co pomogło | MVP |
| E-3 | **Lęk / wyzwalacz** | wyzwalacz, intensywność 0–10, objawy fizyczne (lista + własne), czy pojawiło się unikanie, użyta strategia, intensywność po strategii 0–10 | MVP |
| E-4 | **Sen** | pora snu, pobudka, jakość 1–5, liczba wybudzeń, czy był koszmar | MVP |
| E-5 | **Koszmar (IRT)** | treść koszmaru (opcjonalnie ukryta domyślnie), przepisana wersja, daty ćwiczenia nowej wersji | v1 |
| E-6 | **Notatka swobodna** | tylko tekst | MVP |

| ID | Funkcja | Priorytet |
|---|---|---|
| E-10 | **Szybki wpis**: maks. 2 stuknięcia (typ, intensywność), resztę można uzupełnić później. Wpis oznaczony jako „do uzupełnienia” | MVP |
| E-11 | Edycja i usuwanie wpisów (usunięcie z potwierdzeniem) | MVP |
| E-12 | Tagi: tworzenie, zmiana nazwy, łączenie, kolor | MVP |
| E-13 | Oś czasu z filtrami: typ, tagi, zakres dat, wyszukiwanie pełnotekstowe (w pamięci po odblokowaniu) | MVP |
| E-14 | Widok kalendarza z kolorem dnia według średniego samopoczucia | v1 |
| E-15 | **Szybki wpis bez odblokowania**: dopisanie wpisu bez możliwości odczytu istniejących (patrz §6.4) | v1 |
| E-16 | Ukrywanie treści wybranych wpisów do czasu kliknięcia („treść trudna”) | v1 |

### 4.3. Plan na trudny moment

Struktura oparta na modelu planu bezpieczeństwa Stanley–Brown.

| ID | Funkcja | Priorytet |
|---|---|---|
| P-1 | Sekcje: moje sygnały ostrzegawcze; co mogę zrobić sam(a); miejsca i osoby, które mnie uspokajają; do kogo mogę zadzwonić; profesjonaliści; jak zadbać o bezpieczne otoczenie | MVP |
| P-2 | Dostęp do planu jednym przyciskiem z ekranu głównego | MVP |
| P-3 | Telefony kryzysowe (konfigurowalne według kraju; domyślnie PL, patrz §9) widoczne także **bez odblokowania** skarbca. To jedyna treść dostępna bez hasła i jest statyczna, nie pochodzi od użytkownika | MVP |
| P-4 | Ćwiczenia ugruntowania offline: oddech 4-7-8, oddech pudełkowy z animacją, technika 5-4-3-2-1. Dostępne bez odblokowania | MVP |

### 4.4. Wzorce (analiza lokalna)

| ID | Funkcja | Priorytet |
|---|---|---|
| A-1 | Wykres samopoczucia w czasie (tydzień, miesiąc, 3 miesiące) | MVP |
| A-2 | Najczęstsze tagi i wyzwalacze w okresie | v1 |
| A-3 | Zestawienia: samopoczucie a sen poprzedniej nocy, pora dnia, dzień tygodnia | v1 |
| A-4 | Skuteczność strategii: średni spadek intensywności lęku po danej strategii | v1 |

**Zasady językowe dla A-x:** tylko obserwacje, nigdy wnioski kliniczne. Dobrze: „W dniach po śnie ocenionym na 1–2 twoje samopoczucie było średnio niższe”. Źle: „Masz problem ze snem” albo „Twój stan się pogarsza”. Przy małej liczbie danych (< 14 wpisów w okresie) nie pokazujemy zestawień.

### 4.5. Eksport, kopie, usuwanie

| ID | Funkcja | Priorytet |
|---|---|---|
| X-1 | **Kopia zaszyfrowana**: pojedynczy plik `.mossbook` zaszyfrowany tym samym kluczem (do otwarcia hasłem lub kluczem odzyskiwania). Zapis przez systemowe okno wyboru miejsca | MVP |
| X-2 | Import kopii: scalenie (bez duplikatów, według ID) albo zastąpienie | MVP |
| X-3 | **Raport dla terapeuty (PDF)**: wybór okresu, typów i tagów; podgląd przed zapisem; ostrzeżenie, że PDF **nie jest zaszyfrowany** | v1 |
| X-4 | Eksport jawny JSON (pełna przenośność), z ostrzeżeniem | v1 |
| X-5 | Usunięcie wszystkich danych (§V-9) | MVP |

### 4.6. Przypomnienia

| ID | Funkcja | Priorytet |
|---|---|---|
| R-1 | Lokalne przypomnienia o wybranych porach, domyślnie **wyłączone** | v1 |
| R-2 | Treść neutralna i konfigurowalna, nigdy z danymi z wpisów | v1 |
| R-3 | Brak przypomnień „nadrabiających” i komunikatów o przerwie | v1 |

### 4.7. Języki i dostępność

- Polski, angielski i ukraiński od MVP; reszta przez społeczność (pliki tłumaczeń w repo).
- Tryb ciemny i jasny według systemu, duże cele dotyku (min. 48 px), obsługa czytników ekranu, brak animacji przy `prefers-reduced-motion`.

---

## 5. Model danych

Wszystkie poniższe struktury istnieją **tylko** w pamięci po odblokowaniu lub jako zaszyfrowane bloby. Serializacja: `serde` + CBOR (`ciborium`).

```rust
pub struct Entry {
    pub id: Uuid,                    // UUIDv7
    pub schema: u16,                 // wersja schematu rekordu
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub occurred_at: OffsetDateTime, // kiedy zdarzenie miało miejsce
    pub kind: EntryKind,
    pub mood: Option<u8>,            // 1..=5
    pub intensity: Option<u8>,       // 0..=10
    pub tags: Vec<Uuid>,
    pub note: String,
    pub incomplete: bool,            // szybki wpis do uzupełnienia
    pub sensitive: bool,             // ukryj treść do kliknięcia
}

pub enum EntryKind {
    Mood   { energy: Option<u8>, emotions: Vec<Emotion> },
    Event  { what: String, place: String, body: String, thoughts: String, helped: String },
    Fear   { trigger: String, symptoms: Vec<Symptom>, avoidance: Option<bool>,
             strategy: Option<String>, intensity_after: Option<u8> },
    Sleep  { bedtime: Option<OffsetDateTime>, wake: Option<OffsetDateTime>,
             quality: Option<u8>, awakenings: Option<u8>, nightmare: bool },
    Nightmare { content: String, rescripted: String, rehearsals: Vec<OffsetDateTime> },
    Note,
}

pub struct Tag { pub id: Uuid, pub name: String, pub color: String }

pub struct SafetyPlan {
    pub warning_signs: Vec<String>,
    pub self_coping: Vec<String>,
    pub calming_people_places: Vec<String>,
    pub contacts: Vec<Contact>,
    pub professionals: Vec<Contact>,
    pub safe_environment: Vec<String>,
}

pub struct Settings {
    pub autolock: AutoLock,
    pub locale: String,
    pub crisis_country: String,
    pub reminders: Vec<Reminder>,
}
```

Emocje i objawy to enumy z kluczami tłumaczeń oraz wariantem `Custom(String)`.

---

## 6. Kryptografia

### 6.1. Hierarchia kluczy

```
hasło ──Argon2id(salt, params)──▶ KEK_pwd ──┐
                                            ├─ wrap ─▶ DEK (losowy, 256 bit)
klucz odzyskiwania (24 słowa, 256 bit) ─────┘           │
   └─HKDF─▶ KEK_rec                                      ├─▶ szyfruje rekordy (XChaCha20-Poly1305)
                                                         └─▶ szyfruje klucz prywatny X25519 (skrzynka szybkich wpisów)
```

- **DEK** (data encryption key) jest losowy i nigdy się nie zmienia. Zmiana hasła oznacza tylko ponowne opakowanie DEK nowym KEK, bez przeszyfrowania danych.
- **Argon2id**: domyślnie `m = 64 MiB, t = 3, p = 1`. Parametry zapisane w nagłówku skarbca, żeby można je było podnieść w przyszłości. Kalibracja przy zakładaniu: cel ~0,5–1 s na urządzeniu.
- **Klucz odzyskiwania**: 256 losowych bitów zakodowanych jako 24 słowa BIP-39 (wordlist angielski). KEK_rec = HKDF-SHA256(entropy, info = `"mossbook/recovery/v1"`).
- **Opakowanie kluczy (wrap)**: XChaCha20-Poly1305 z losowym nonce, AAD = `"mossbook/wrap/v1" || vault_id`.

### 6.2. Szyfrowanie rekordów

- Każdy rekord: `nonce (24 B, losowy) || ciphertext || tag`.
- AAD = `"mossbook/rec/v1" || vault_id || record_id || record_type`, co wiąże blob z miejscem w bazie i uniemożliwia podmianę rekordów między sobą.
- Typ rekordu w AAD: `entry`, `tag`, `plan`, `settings`.

### 6.3. Zarządzanie sekretami w pamięci

- Klucze w `secrecy::SecretBox` / `zeroize::Zeroizing`; zerowanie przy blokadzie i zamknięciu.
- **Frontend nigdy nie dostaje klucza.** Webview otrzymuje wyłącznie odszyfrowane obiekty domenowe przez komendy Tauri.
- Hasło z frontendu trafia do Rusta jako `String`, który jest natychmiast przenoszony do `Zeroizing<String>`. Ograniczenia JS (niemożność wyzerowania stringa w webview) dokumentujemy w `SECURITY.md`.

### 6.4. Szybki wpis bez odblokowania (v1)

- Skarbiec ma parę kluczy X25519. Klucz publiczny jest jawny w nagłówku, prywatny zaszyfrowany DEK.
- Szybki wpis bez hasła jest szyfrowany jako **sealed box** (`crypto_box`) kluczem publicznym i trafia do tabeli `inbox`.
- Po odblokowaniu aplikacja odszyfrowuje skrzynkę, przenosi wpisy do `records` (zaszyfrowane DEK) i czyści `inbox`.
- Efekt: bez hasła można **dopisać** wpis, ale nie można niczego **odczytać**.

### 6.5. Biometria (v1)

- Przy włączaniu: DEK opakowany kluczem z bezpiecznego magazynu systemu (Android Keystore z `setUserAuthenticationRequired`, iOS Keychain z `biometryCurrentSet`, przez `tauri-plugin-biometric` lub własny plugin).
- Zmiana zestawu odcisków lub twarzy unieważnia klucz i wymaga hasła.
- Desktop: biometria w MVP pominięta; tylko hasło.

### 6.6. Biblioteki (Rust, RustCrypto)

| Cel | Crate |
|---|---|
| AEAD | `chacha20poly1305` (XChaCha20Poly1305) |
| KDF z hasła | `argon2` |
| KDF klucza | `hkdf` + `sha2` |
| Sealed box | `crypto_box` (feature `seal`) |
| Mnemonik | `bip39` |
| Losowość | `rand_core::OsRng` / `getrandom` |
| Sekrety | `secrecy`, `zeroize` |

**Zakaz własnych prymitywów kryptograficznych.** Tylko złożenie gotowych.

---

## 7. Przechowywanie

Plik `vault.db` (SQLite przez `rusqlite` z feature `bundled`) w katalogu danych aplikacji.

```sql
CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value BLOB NOT NULL
);
-- klucze: format_version, vault_id, kdf_params, kdf_salt,
--         wrapped_dek_pwd, wrapped_dek_rec, x25519_pub, wrapped_x25519_priv

CREATE TABLE records (
  id     BLOB PRIMARY KEY,   -- UUID (16 B)
  type   INTEGER NOT NULL,   -- 1 entry, 2 tag, 3 plan, 4 settings
  blob   BLOB NOT NULL       -- nonce || ciphertext || tag
);

CREATE TABLE inbox (
  id     BLOB PRIMARY KEY,
  blob   BLOB NOT NULL       -- sealed box
);
```

- **Żadnych jawnych dat ani pól wyszukiwania w kolumnach.** Po odblokowaniu wszystkie rekordy ładowane są do pamięci; filtrowanie i wyszukiwanie odbywa się w Ruście. Przy realnych wolumenach (kilka tysięcy wpisów, kilka MB) to szybkie i proste.
- `PRAGMA journal_mode = WAL`, `PRAGMA secure_delete = ON`.
- Po masowym usunięciu: `VACUUM`.
- Migracje schematu SQL w `core/migrations/`; migracje schematu rekordów przez pole `schema` w `Entry`.

**Format kopii `.mossbook`:** archiwum z nagłówkiem (`magic = "MOSSBOOK"`, wersja formatu, `vault_id`, parametry KDF, opakowane DEK) i zaszyfrowanymi rekordami. Format opisany w `docs/backup-format.md`, żeby dało się go odczytać niezależnym narzędziem.

---

## 8. Architektura aplikacji

### 8.1. Stack

- **Tauri 2**: desktop (Linux, Windows, macOS) i mobile (Android, iOS) z jednego kodu.
- **Rust**: cała logika domenowa, kryptografia, przechowywanie, raporty PDF.
- **Svelte 5 + TypeScript + Vite**: wyłącznie warstwa prezentacji.
- Wykresy: `uPlot` (lekki, bez zależności), zainstalowany lokalnie, bez CDN.
- PDF: `genpdf` lub `printpdf` w Ruście, generowany w core, zapisywany przez systemowe okno zapisu.

### 8.2. Struktura repozytorium

```
mossbook/
├── core/                    # crate `mossbook-core`: czysty Rust, bez Tauri
│   ├── src/
│   │   ├── crypto/          # kdf.rs, aead.rs, wrap.rs, recovery.rs, sealed.rs
│   │   ├── vault/           # create.rs, unlock.rs, lock.rs, rekey.rs
│   │   ├── store/           # sqlite.rs, migrations.rs
│   │   ├── domain/          # entry.rs, tag.rs, plan.rs, settings.rs
│   │   ├── query/           # filter.rs, search.rs, stats.rs
│   │   ├── backup/          # export.rs, import.rs, format.rs
│   │   └── report/          # pdf.rs
│   ├── migrations/
│   └── tests/
├── app/
│   ├── src-tauri/           # komendy Tauri, stan, uprawnienia (capabilities), konfiguracja platform
│   └── src/                 # Svelte 5
│       ├── lib/api/         # typowane wrappery na invoke()
│       ├── lib/components/
│       ├── lib/i18n/        # pl.json, en.json, uk.json
│       └── routes/          # lock, setup, timeline, entry, plan, patterns, settings
├── docs/
│   ├── threat-model.md
│   ├── backup-format.md
│   └── crypto.md
├── SECURITY.md
├── CLAUDE.md
└── README.md
```

**Zasada:** `core` nie zależy od Tauri i jest testowany niezależnie. `src-tauri` to cienka warstwa: mapuje komendy na funkcje core i trzyma stan.

### 8.3. Stan i komendy Tauri

```rust
pub struct AppState {
    vault: Mutex<VaultState>,
}

pub enum VaultState {
    NoVault,
    Locked { header: VaultHeader },
    Unlocked(UnlockedVault), // DEK + rekordy w pamięci; Drop zeruje klucze
}
```

Komendy (wszystkie zwracają `Result<T, AppError>`, gdzie `AppError` ma kod i komunikat bez danych użytkownika):

| Komenda | Opis |
|---|---|
| `vault_status` | `NoVault` / `Locked` / `Unlocked` |
| `vault_create(password) -> RecoveryPhrase` | Tworzy skarbiec, zwraca 24 słowa (jednorazowo) |
| `vault_confirm_recovery(words)` | Weryfikacja przepisanych słów |
| `vault_unlock(password)` / `vault_unlock_recovery(phrase, new_password)` | |
| `vault_lock` | |
| `vault_change_password(old, new)` | |
| `vault_destroy(password)` | |
| `entries_list(filter) -> Vec<EntryView>` | |
| `entry_get(id)` / `entry_save(entry)` / `entry_delete(id)` | |
| `quick_entry(kind, intensity)` | Działa także, gdy skarbiec zablokowany (v1, sealed box) |
| `tags_*`, `plan_get`, `plan_save`, `settings_get`, `settings_save` | |
| `stats(range) -> Stats` | |
| `backup_export(path)` / `backup_import(path, mode)` | |
| `report_pdf(params, path)` | v1 |

Auto-blokada: zdarzenie cyklu życia okna/aplikacji (utrata fokusu / przejście w tło) wywołuje `vault_lock` po stronie Rusta. Timer bezczynności jest w Ruście, a frontend tylko go resetuje.

### 8.4. Bezpieczeństwo platform

| Platforma | Działanie |
|---|---|
| Wszystkie | CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:; style-src 'self' 'unsafe-inline'`; brak pluginu `http`; capabilities Tauri ograniczone do niezbędnych komend; czcionki i zasoby wyłącznie lokalne |
| Android | Usunięcie `android.permission.INTERNET` z manifestu wydania (`tools:node="remove"`, w buildzie debug potrzebne dla dev serwera); `android:allowBackup="false"` + `dataExtractionRules` wykluczające wszystko; `FLAG_SECURE` w `MainActivity` (gen/android) |
| iOS | Wykluczenie katalogu danych z kopii (`isExcludedFromBackup`); zasłona widoku przy `willResignActive`; ochrona plików `NSFileProtectionComplete` |
| Desktop | Zasłona okna przy utracie fokusu (opcjonalna), dane w katalogu aplikacji użytkownika |

Do weryfikacji w implementacji: dokładne miejsca konfiguracji natywnej w projekcie Tauri 2 (pliki w `gen/android` i `gen/apple`) oraz to, czy zmiany przetrwają `tauri android init` / `tauri ios init`.

### 8.5. UI i UX

- **Ekran blokady**: pole hasła oraz dwa przyciski dostępne bez hasła: „Plan na trudny moment / telefony” (tylko numery kryzysowe i ćwiczenia, bez danych użytkownika) i „Szybki wpis” (v1).
- **Ekran główny**: duży przycisk szybkiego wpisu, oś czasu, przycisk blokady.
- Spokojna paleta, brak czerwieni jako koloru ostrzeżeń przy treściach emocjonalnych, brak wyskakujących okien bez potrzeby.
- Każdy formularz da się zapisać po wypełnieniu jednego pola.
- Teksty pisane prostym, ciepłym językiem, w drugiej osobie, bez żargonu klinicznego.

---

## 9. Treści wrażliwe

- **Telefony kryzysowe (PL, do weryfikacji przed każdym wydaniem):**
  - 112: numer alarmowy,
  - 116 123: telefon zaufania dla dorosłych w kryzysie emocjonalnym,
  - 800 70 2222: Centrum Wsparcia dla Osób Dorosłych w Kryzysie Psychicznym (całodobowo),
  - 116 111: telefon zaufania dla dzieci i młodzieży.
- Numery trzymane w pliku danych `core/data/crisis/{kraj}.json`, ze źródłem i datą weryfikacji. Wkład społeczności dla innych krajów przez PR.
- Komunikat w ustawieniach i na pierwszym ekranie: aplikacja nie zastępuje pomocy specjalisty; w nagłym zagrożeniu zadzwoń pod 112.
- Teksty ćwiczeń ugruntowania i opisy IRT warto skonsultować z psychoterapeutą przed wydaniem v1.

---

## 10. Testy i jakość

- **Kryptografia**: testy round-trip, błędne hasło, błędny klucz odzyskiwania, podmiana bajtu w blobie (musi dać błąd), zamiana rekordów miejscami (AAD musi to wykryć), zmiana hasła zachowuje dane, stałe wektory testowe dla formatu.
- **Testy własności** (`proptest`): serializacja i szyfrowanie dowolnych `Entry`.
- **Kopie**: eksport → import do pustego skarbca → identyczne dane; import starszej wersji formatu.
- **Migracje**: baza z każdej poprzedniej wersji otwiera się poprawnie.
- **Brak sieci**: test CI sprawdzający, że w drzewie zależności nie ma `reqwest`, `hyper`, `ureq` itp. (`cargo deny` z listą zakazanych crate'ów); test sprawdzający manifest Androida wydania.
- **Brak wycieków do logów**: `AppError` i `Debug` dla typów domenowych nie wypisują treści (ręczna implementacja `Debug` z redakcją).
- **Frontend**: `svelte-check`, testy komponentów (Vitest), testy e2e scenariuszy: założenie skarbca, wpis, blokada, odblokowanie.
- **Dane testowe** wyłącznie syntetyczne, generowane w kodzie.

CI (GitHub Actions): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`, `cargo audit`, `npm run check`, `npm test`, build desktop dla trzech systemów, build APK.

---

## 11. Dystrybucja i licencja

- **Licencja**: `GPL-3.0-or-later` z dodatkowym zezwoleniem (sekcja 7) na dystrybucję przez sklepy z aplikacjami, patrz `LICENSE-EXCEPTION.md`. Copyleft zapobiega zamkniętym forkom z dodaną telemetrią.
- **F-Droid**: aplikacja bez sieci i bez zależności od Google to idealny kandydat. Wymaga powtarzalnego buildu.
- GitHub Releases: instalatory desktop + APK, z sumami kontrolnymi.
- Google Play i App Store później. Uwaga na kategorię „zdrowie” w sklepach i ich wymagania dotyczące deklaracji o danych (u nas: „nie zbieramy żadnych danych”).

---

## 12. Kolejność implementacji

1. **Szkielet**: workspace Cargo (`core`, `app/src-tauri`), Svelte 5 + Vite, CI z fmt/clippy/test/deny. Pusta aplikacja uruchamia się na desktopie.
2. **Core: krypto**: `kdf`, `aead`, `wrap`, `recovery` z pełnymi testami. Bez UI.
3. **Core: skarbiec i magazyn**: create/unlock/lock/rekey, SQLite, tabele, migracje, testy integracyjne.
4. **Core: domena**: `Entry`, `Tag`, `SafetyPlan`, `Settings`, CRUD, filtr, wyszukiwanie.
5. **Tauri: komendy i stan**, auto-blokada po stronie Rusta.
6. **UI: zakładanie skarbca i blokada** (z kluczem odzyskiwania i potwierdzeniem).
7. **UI: szybki wpis + pełny formularz + oś czasu.**
8. **UI: plan na trudny moment + telefony + ćwiczenia** (dostępne z ekranu blokady).
9. **Kopia `.mossbook`**: eksport i import.
10. **Wykres samopoczucia (A-1)**, i18n pl/en/uk.
11. **Android**: build, usunięcie INTERNET, `FLAG_SECURE`, wyłączenie kopii. Test na urządzeniu.
12. **Wydanie MVP 0.1.0**, `SECURITY.md`, `docs/threat-model.md`.
13. v1: biometria, szybki wpis bez odblokowania, IRT, raport PDF, statystyki A-2–A-4, przypomnienia, iOS.

---

## 13. Instrukcje dla Claude Code (do wklejenia w `CLAUDE.md`)

```markdown
# Mossbook: zasady dla agenta

## Nienaruszalne
- NIGDY nie dodawaj zależności sieciowych (reqwest, hyper, ureq, tauri-plugin-http, fetch do zewnętrznych adresów, CDN). Aplikacja działa w 100% offline.
- NIGDY nie zapisuj danych użytkownika na dysk w postaci jawnej. Wszystko przez warstwę `core::store` i szyfrowanie z `core::crypto`.
- NIGDY nie loguj treści wpisów, tagów, haseł, kluczy ani fraz odzyskiwania (`println!`, `log::*`, `tracing::*`, `console.log`). Typy domenowe mają ręczny `Debug` z redakcją.
- NIGDY nie implementuj własnych prymitywów kryptograficznych. Używaj crate'ów RustCrypto wymienionych w SPEC.md §6.6.
- Klucze trzymaj w `secrecy`/`zeroize`. Frontend nigdy nie dostaje klucza.
- Nie dodawaj telemetrii, analityki ani raportowania błędów na zewnątrz.
- Teksty w UI: bez diagnoz i ocen klinicznych, bez gamifikacji, bez komunikatów wywołujących poczucie winy.

## Architektura
- Logika w `core/` (czysty Rust, bez Tauri). `app/src-tauri` to cienka warstwa komend.
- Svelte 5 (runes) + TypeScript. Komunikacja wyłącznie przez typowane wrappery w `app/src/lib/api/`.
- Specyfikacja: SPEC.md. Przy wątpliwościach pytaj, zamiast zgadywać, szczególnie w kryptografii i formacie plików.

## Jakość
- Każda zmiana w `core/crypto`, `core/vault`, `core/backup` wymaga testów (w tym negatywnych).
- Przed zakończeniem zadania: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `npm run check`.
- Dane testowe tylko syntetyczne.
- Zmiana formatu rekordu lub kopii = podbicie wersji + migracja + test migracji.
```

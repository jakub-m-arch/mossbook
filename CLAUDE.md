# Mossbook: zasady dla agenta

Specyfikacja: `SPEC.md`. Plan i stan prac: `docs/PLAN.md` (aktualizuj checklistę po każdym etapie).

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
- Przy wątpliwościach pytaj, zamiast zgadywać, szczególnie w kryptografii i formacie plików. Decyzje podjęte poza SPEC.md są w `docs/PLAN.md` §3.

## Jakość
- Każda zmiana w `core/crypto`, `core/vault`, `core/backup` wymaga testów (w tym negatywnych).
- Przed zakończeniem zadania: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm run check` (w `app/`).
- Dane testowe tylko syntetyczne.
- Zmiana formatu rekordu lub kopii = podbicie wersji + migracja + test migracji.

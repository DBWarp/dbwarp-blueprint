# Współtworzenie

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i może zawierać błędy. Nie powinna być traktowana jako tekst kontraktowy. Zobacz [kanoniczne źródło angielskie](../../CONTRIBUTING.md).

**Języki:** [English](../../CONTRIBUTING.md) | [Deutsch](../de/CONTRIBUTING.md) | [Français](../fr/CONTRIBUTING.md) | [Español](../es/CONTRIBUTING.md) | **Polski** | [日本語](../ja/CONTRIBUTING.md) | [简体中文](../zh/CONTRIBUTING.md)

Zacznij od zgłoszenia bez danych wrażliwych, opisującego problem i niewielkie
syntetyczne odtworzenie. W przypadku istotnych zmian omów podejście przed
przygotowaniem poprawki. Opiekunowie decydują, czy propozycja pasuje do produktu
i jego granic bezpieczeństwa; otwarcie zgłoszenia lub pull requestu nie oznacza
akceptacji.

Bezpieczne zgłaszanie problemów opisuje [SUPPORT.md](SUPPORT.md), a prywatne
zgłaszanie podatności — [SECURITY.md](SECURITY.md). Prowadź dyskusję z szacunkiem
i skupiaj się na zachowaniu, które można odtworzyć.

## Przygotowanie zmiany

- Używaj przypiętego zestawu narzędzi i zablokowanych wersji zależności opisanych
  w [BUILD.md](BUILD.md).
- Ogranicz zakres zmian i dodaj testy regresji dla zmienionego zachowania.
- Nigdy nie umieszczaj danych klientów, poświadczeń, szczegółów prywatnej
  infrastruktury ani identyfikujących dowodów audytu w poprawce lub zestawie
  danych testowych.
- Zachowaj jawną zgodę, ograniczony wpływ na źródło, dostęp z minimalnymi
  uprawnieniami oraz rzetelne raportowanie brakujących lub zdegradowanych
  obserwacji.
- Dokumentuj każdą zmianę kontraktu Blueprint lub kodowania kompresji. Istniejące
  reguły pól opcjonalnych i zgodności są częścią interfejsu.
- Zachowaj kanoniczny angielski w opcjach CLI, kodach diagnostycznych i polach
  serializowanych. Zmiany tekstu widocznego dla użytkownika podczas działania
  muszą aktualizować każdy dostarczany katalog komunikatów. Angielski Markdown
  jest wiążący; tłumaczony Markdown ma charakter uzupełniający.

## Kontrole lokalne

W repozytorium źródłowym, po zainstalowaniu przypiętego zestawu narzędzi:

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

Współdzielony rdzeń ma własny zestaw testów jednostkowych:

```bash
cargo test --locked --manifest-path crates/dbwarp-blueprint-core/Cargo.toml --lib
```

Pomyślne testy lokalne nie stanowią kwalifikacji wersji baz danych ani platform.
Opisz, co faktycznie przetestowano, i jawnie oznacz pozostałe konfiguracje jako
nieprzetestowane. Bez zgody opiekunów nie publikuj artefaktów, nie aktualizuj
tagów wydań ani nie zmieniaj ustawień bezpieczeństwa repozytorium w ramach
poprawki.

## Przepływ pracy opiekuna

Kanonicznym źródłem jest angielska pomoc Rust oraz definicje komunikatów/UI
w `src/i18n.rs`. Gdy zmienia się dowolny tekst widoczny dla klienta:

1. zaktualizuj w tym samym commicie każdy katalog językowy w `locales/`;
2. zachowaj dokładnie wszystkie symbole zastępcze i kanoniczne tokeny operacyjne;
3. uruchom ukierunkowany test dokładnego pokrycia;
4. dodaj lub zaktualizuj odpowiedni przypadek granicy operatorskiej w
   `tests/cli_errors.rs`, gdy zmienia się awaria lub ostrzeżenie;
5. uruchom pełny zestaw testów i sprawdź reprezentatywne dane wyjściowe pomocy/prezentacji;
6. uzyskaj weryfikację techniczną przez osobę biegle posługującą się danym językiem, zanim nowe brzmienie zostanie uznane za ostateczne dla umowy z klientem, zgłoszenia regulacyjnego lub publicznego materiału marketingowego.

Ten proces dokładnego pokrycia dotyczy katalogów używanych podczas działania,
wbudowanych w plik binarny. Tłumaczony Markdown ma charakter uzupełniający;
zobacz [`docs/TRANSLATIONS.md`](../TRANSLATIONS.md).

Ukierunkowana walidacja:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

Testy integracyjne dowodzą również, że tokeny opcji są identyczne we wszystkich
językach, zlokalizowane kody DBP pozostają stabilne, emitowany TOML nie zależy od
języka, a treść wygenerowanej prezentacji zawiera wybrane ustawienia regionalne.

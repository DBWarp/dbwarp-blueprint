# Wizualna prezentacja podsumowująca

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../DECK.md).

[English](../../DECK.md) | [Deutsch](../de/DECK.md) | [Français](../fr/DECK.md) | [Español](../es/DECK.md) | [Polski](DECK.md) | [日本語](../ja/DECK.md) | [简体中文](../zh/DECK.md)

`dbwarp-blueprint --deck blueprint.pptx` zapisuje opcjonalne podsumowanie Blueprint w
programie PowerPoint (`.pptx`) obok pliku TOML wskazanego przez `--out`.
`dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` tworzy później tę samą
prezentację z istniejącego, sprawdzonego pliku Blueprint, bez łączenia się z bazą
danych. Jest to prezentacja tych samych zanonimizowanych danych — żadne
dodatkowe informacje o bazie danych nie są odczytywane, wysyłane ani obliczane.

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx \
  --lang ja
```

`--lang en|de|fr|es|pl|ja|zh` lokalizuje tekst prezentacji przeznaczony dla
człowieka oraz metadane językowe programu PowerPoint. Anonimowe identyfikatory,
nazwy typów baz danych, metody indeksowania, pomiary i źródłowy plik TOML
pozostają kanoniczne oraz neutralne językowo. Walidacja katalogu kończy się
błędem zamiast zastępować brakującą frazę prezentacji tekstem angielskim. Zobacz
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## Stopka i poufność

Każdy slajd z treścią używa firmowej stopki DBWarp: małego logo po lewej,
opcjonalnego separatora i poziomu poufności, samego wyśrodkowanego numeru slajdu
oraz `DBWarp.com` po prawej. Slajd tytułowy pozostaje bez numeru.

Opcja `--deck-confidentiality public|internal|confidential|restricted` dodaje
jedną ze zlokalizowanych, wbudowanych etykiet klasyfikacji. Każda inna bezpieczna
i niepusta wartość jest etykietą niestandardową oraz jest wyświetlana bez zmian;
wartości ze spacjami należy ująć w cudzysłów, na przykład
`--deck-confidentiality "CLIENT // SENSITIVE"`. Etykiety nie mogą zawierać
spacji na początku ani na końcu, znaków sterujących lub dwukierunkowego
formatowania ani przekraczać 48 jednostek szerokości wyświetlania. Pomiń tę
opcję, aby nie wyświetlać etykiety. Ustawienie zmienia wyłącznie prezentację;
nie zmienia pliku Blueprint ani danych podsumowanych w prezentacji. Przy
ustalonym `--generated-at` wynik pozostaje deterministyczny.

## Właściwości zaufania

- **Tworzona lokalnie, z pamięci.** Prezentacja jest renderowana z tego samego
  Blueprint w pamięci, który tworzy `blueprint.toml`. Nie jest wykonywane dodatkowe
  zapytanie do bazy danych ani drugi przebieg po katalogu. W trybie `--from-toml`
  Blueprint w pamięci jest zamiast tego ładowany ze sprawdzonego pliku TOML.
- **Bez sieci.** Generowanie prezentacji nie nawiązuje żadnego połączenia
  wychodzącego.
- **Bez bibliotek stron trzecich.** OOXML jest tworzony bezpośrednio w
  `src/deck.rs`; plik `.pptx` jest zwykłym archiwum ZIP części XML, które można
  rozpakować poleceniem `unzip` i odczytać. Bez automatyzacji programu PowerPoint,
  usługi renderowania ani dodatkowego pakietu crate w grafie zależności.
  Zatwierdzone obrazy logo DBWarp i statyczne kroje DM Sans są wbudowane w
  binarkę Rust i zapisywane jako części multimedialne i fontowe OOXML;
  generowanie nie odczytuje ścieżki zasobu w czasie działania.
- **Bez rzeczywistych identyfikatorów i danych wierszy.** Tabele, kolumny i
  indeksy występują jako te same anonimowe symbole zastępcze co w pliku Blueprint
  (`table-001`, `col-1`, `idx-1`, `schema-A`), a każda liczba ma tę samą
  udokumentowaną dokładność. Prezentacja nie zawiera faktów specyficznych dla
  klienta poza tymi, które znajdują się w pliku Blueprint.
- **Deterministyczna.** Przy ustalonej wartości `--generated-at` ten sam Blueprint
  tworzy identyczny bajtowo plik `.pptx` dla tego samego wybranego języka (stała
  kolejność części i stałe znaczniki czasu).

## Co zawiera

Prezentacja dostosowuje się do rozmiaru schematu:

- **Tytuł** — logo i hasło DBWarp, silnik, wersja, rodzaj źródła, liczba tabel i
  znacznik czasu wygenerowania.
- **Podsumowanie zarządcze** — sygnały dla kierownictwa dotyczące skali
  migracji, koncentracji danych, złożoności relacji i dowodów gotowych do
  udostępnienia.
- **Przegląd** — sumy tabel, wierszy, rozmiaru danych i rozmiaru indeksów, a
  także liczby kolumn, indeksów, kluczy obcych i schematów.
- **Małe schematy** (kilka tabel) — panel o dopasowanym rozmiarze dla każdej
  tabeli (wiersze, bajty, typy kolumn, indeksy) oraz diagram kluczy obcych.
- **Duże schematy** — charakterystyka zamiast wyliczenia:
  - *Największe tabele*: największe tabele według rozmiaru, z pozostałą liczbą
    `+ N more`.
  - *Skład schematu*: rozkład typów kolumn oraz statystyki indeksów i całości.
  - *Relacje*: liczba kluczy obcych, tabele połączone i samodzielne oraz
    najczęściej wskazywane tabele centralne.
- **Zmierzona kompresja** (tylko Poziom 2) — liczba próbkowanych tabel, ważony
  współczynnik zstd-3, przewidywany rozmiar po kompresji i najbardziej podatne
  na kompresję spośród próbkowanych tabel.
- **Model zaufania** — slajd końcowy podsumowujący powyższe gwarancje.

## Przeglądanie danych wyjściowych

Plik `.pptx` jest standardowym pakietem OOXML. Aby sprawdzić dokładnie, co
zawiera:

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

Otwórz go w programie PowerPoint, LibreOffice Impress lub Google Slides.
Generator znajduje się w [`src/deck.rs`](https://github.com/DBWarp/dbwarp-blueprint/blob/main/src/deck.rs) i jest wbudowany w
plik binarny Rust. Nie ma oddzielnego generatora prezentacji, który trzeba
instalować, audytować lub utrzymywać w synchronizacji.

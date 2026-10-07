# Wizualna prezentacja podsumowująca

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../DECK.md).

[English](../../DECK.md) | [Deutsch](../de/DECK.md) | [Français](../fr/DECK.md) | [Español](../es/DECK.md) | [Polski](DECK.md) | [日本語](../ja/DECK.md) | [简体中文](../zh/DECK.md)

`dbwarp-blueprint --deck blueprint.pptx` zapisuje opcjonalne podsumowanie Blueprint w
programie PowerPoint (`.pptx`) obok pliku TOML wskazanego przez `--out`.
`dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx` tworzy później tę samą
prezentację z istniejącego, sprawdzonego pliku Blueprint, bez łączenia się z bazą
danych. Jest to prezentacja tych samych zanonimizowanych danych — nic więcej nie
jest odczytywane z bazy danych ani do niej wysyłane. Prezentacja oblicza wyłącznie
udokumentowane lokalne podsumowania i prognozy z pól już obecnych w Blueprint.

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

Każdy slajd zawiera również lokalizowane notatki dla prelegenta. Notatki przekształcają widoczne dane w krótkie, naturalne podsumowanie, zamiast powtarzać każdą etykietę i wartość na slajdzie. Używają tylko pomiarów z Blueprint i pomijają powtarzający się nagłówek, dzięki czemu widok prezentera i wydrukowane notatki dodają interpretację, nie wprowadzając nowych faktów.

## Stopka i poufność

Każdy slajd zawiera ten sam nagłówek: mały logo po lewej stronie, opcjonalny separator i poziom poufności, numer slajdu pośrodku oraz `DBWarp.com` po prawej stronie. Slajd tytułowy nie jest ponumerowany.

Opcja `--deck-confidentiality public|internal|confidential|restricted` dodaje
jedną ze zlokalizowanych, wbudowanych etykiet klasyfikacji. Każda inna bezpieczna
i niepusta wartość jest etykietą niestandardową oraz jest wyświetlana bez zmian;
wartości ze spacjami należy ująć w cudzysłów, na przykład
`--deck-confidentiality "CLIENT // SENSITIVE"`. Etykiety nie mogą zawierać
spacji na początku ani na końcu, znaków sterujących lub dwukierunkowego
formatowania ani przekraczać 48 jednostek szerokości wyświetlania. Pomiń tę
opcję, aby nie wyświetlać etykiety. Ustawienie zmienia wyłącznie prezentację;
nie zmienia pliku Blueprint ani danych podsumowanych w prezentacji. Przy
dokładnie tym samym sprawdzonym Blueprint, języku, etykiecie i znaczniku czasu
bajty prezentacji są powtarzalne.

## Właściwości zaufania

- **Tworzona lokalnie, z pamięci.** Prezentacja jest renderowana z tego samego
  Blueprint w pamięci, który tworzy `blueprint.toml`. Nie jest wykonywane dodatkowe
  zapytanie do bazy danych ani drugi przebieg po katalogu. W trybie `--from-toml`
  Blueprint w pamięci jest zamiast tego ładowany ze sprawdzonego pliku TOML.
- **Bez sieci aplikacji.** Generowanie prezentacji nie otwiera żadnego połączenia
  sieciowego; odczyt Blueprint ze ścieżki zamontowanej w sieci nadal podlega
  stosowi pamięci masowej hosta.
- **Bez bibliotek stron trzecich.** Moduł zapisujący OOXML jest zaimplementowany
  w `src/deck.rs` i jego modułach `deck_*`. Plik `.pptx` jest archiwum ZIP
  złożonym z części XML, które można sprawdzić za pomocą `unzip`. Nie używa
  automatyzacji PowerPoint, usługi renderowania ani dodatkowej zależności.
  Logotypy DBWarp i statyczne czcionki DM Sans są osadzone w binarnym pliku Rust
  i zapisywane jako fragmenty media/font w formacie OOXML; generowanie nie
  odczytuje ścieżki do zasobu w czasie działania.
- **Bez rzeczywistych identyfikatorów i danych wierszy.** Tabele, kolumny i
  indeksy występują jako te same anonimowe symbole zastępcze co w pliku Blueprint
  (`table-001`, `col-1`, `idx-1`, `schema-A`). Pomiary źródłowe zachowują
  udokumentowaną dokładność; każda prognoza jest obliczana wyłącznie z pól już
  obecnych w Blueprint. Poza tym wejściem prezentacja nie zawiera niczego
  specyficznego dla Twojej bazy danych.
- **Powtarzalna ze stałego wejścia.** Ten sam sprawdzony Blueprint tworzy
  identyczny bajtowo plik `.pptx` dla tego samego języka, etykiety poufności i
  ustalonego znacznika czasu (stała kolejność części i znaczniki czasu). Nie
  oznacza to identyczności dwóch przechwyceń na żywo: wymagają one także tego
  samego chronionego `--anonymization-key-file`, stanu źródła i opcji przechwycenia.

## Co zawiera

Prezentacja dostosowuje się do rozmiaru schematu:

- **Tytuł** — logo i hasło DBWarp, silnik, wersja, rodzaj źródła, liczba tabel i
  znacznik czasu wygenerowania.
- **Podsumowanie zarządcze** — sygnały dla kierownictwa dotyczące skali
  migracji, koncentracji danych, złożoności relacji i sygnałów dowodowych do
  przeglądu.
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
- **Logika bazy danych wykraczająca poza tabele.** Obiekty nietablicowe (`graph` i `analyzed` zawierają szczegółowe informacje): sześć grup w języku naturalnym wyjaśnia logikę i zależności bazy danych poza zwykłymi definicjami tabel: warstwy zapytań, wykonywalna logika, automatyczne zachowania, typy i obiekty wytwarzające wartości, zewnętrzne zależności oraz konfiguracja platformy. Wyświetlane liczby pochodzą z inwentarza artefaktów Blueprint.
- **Złożoność artefaktów** (tylko dla poziomów szczegółowości `graph` i `analyzed`): łączny poziom złożoności obiektów innych niż tabele, pokrycie populacji obiektów podlegających ocenie oraz poziomy wszystkich siedmiu wymiarów. Slajd definiuje te wymiary jako rozmiar definicji, rozgałęzienia, wykorzystanie funkcji, zależności, wymagania środowiskowe, ukryty kod źródłowy i zachowanie specyficzne dla dialektu. Każdy wymiar pokazuje pokrycie obok poziomu, dlatego częściowe lub nieznane dowody nie mogą wyglądać jak niska złożoność. Ogólny wynik `unknown` oznacza niekompletne dowody; `not applicable` wymaga pustej, udowodnionej jako kompletna populacji obiektów podlegających ocenie.
- **Model zaufania** — slajd końcowy podsumowujący powyższe właściwości zaufania.

## Przeglądanie danych wyjściowych

Plik `.pptx` jest standardowym pakietem OOXML. Aby sprawdzić dokładnie, co
zawiera:

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

Otwórz go w programie PowerPoint, LibreOffice Impress lub Google Slides. Mechanizm tworzenia prezentacji znajduje się w [`src/deck.rs`](../../src/deck.rs) i jest wbudowany w plik binarny Rust. Nie ma oddzielnego narzędzia do tworzenia prezentacji, które trzeba instalować lub audytować.

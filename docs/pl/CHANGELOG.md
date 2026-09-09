# Historia zmian

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i może zawierać błędy. Nie powinna być traktowana jako tekst kontraktowy. Zobacz [kanoniczne źródło angielskie](../../CHANGELOG.md).

**Języki:** [English](../../CHANGELOG.md) | [Deutsch](../de/CHANGELOG.md) | [Français](../fr/CHANGELOG.md) | [Español](../es/CHANGELOG.md) | **Polski** | [日本語](../ja/CHANGELOG.md) | [简体中文](../zh/CHANGELOG.md)

Wersje wydań identyfikują kolektor. Wersja schematu Blueprint i kodowanie próbek
kompresji są odrębnymi kontraktami zgodności; zobacz [FORMAT.md](FORMAT.md) i
[pomiar kompresji](COMPRESSION_MEASUREMENT.md).

## 1.5.1

### Przechwytywanie i wierność

- Zachowuje schemat Blueprint v6 i rozróżnia oszacowania katalogowe, obserwacje
  próbek oraz niedostępne dowody świeżości statystyk.
- Ulepsza pomiar ładunków binarnych, profilowanie już skompresowanych ładunków i
  ograniczone sondy kompresji. Współczynniki dla różnych wartości
  `sample_encoding` nie są zamienne; konsument musi rozpoznawać kodowanie przed
  użyciem pomiaru.
- Ogranicza adaptacyjne ponowienia próbkowania MySQL i SQL Server, w tym
  nadmiernie duże wartości oraz zwiększenie rozmiaru wskutek zestawu znaków.
  Zapisuje pozostałe obciążenie wynikające z prefiksów, zachowując metadane
  pierwotnych długości próbkowanych wartości.
- Poprawia wykrywanie obcięcia w MySQL, gdy zestaw znaków połączenia zmienia
  długość zwracanych danych w bajtach.
- Ulepsza obsługę długości wartości syntetycznych, kardynalności, rozkładu i
  lokalności danych we współdzielonym rdzeniu Blueprint.

### Obsługa i przegląd

- Doprecyzowuje konfigurację dedykowanych kont z minimalnymi uprawnieniami,
  instrukcje budowania ze źródeł, weryfikację przez kompilację porównawczą i
  macierz zweryfikowanych wersji baz danych.
- Odświeża dokumentację tłumaczoną maszynowo oraz komunikaty podczas działania,
  zachowując angielski jako wersję wiążącą, a tłumaczenia jako uzupełnienie.
- Wzmacnia kontrole uprawnień wykonywania publicznych plików źródłowych i archiwów
  wydań.
- Dodaje historię wydań oraz wskazówki dotyczące wsparcia i współtworzenia do
  dystrybucji źródeł i plików binarnych.
- Usuwa nieużywane grafiki ASCII bez zmiany obsługiwanych trybów banera.

### Zgodność i wdrożenie

Nowy kolektor nadal odczytuje istniejące pliki Blueprint. Zgodność w odwrotnym
kierunku nie jest gwarantowana: czytnik w wersji 1.5.0 odrzuca nowe opcjonalne
pole `sample_layout`, a starsi konsumenci mogą odrzucać nowe kodowania próbek
kompresji. Sam parser schematu v6 nie jest więc dowodem zgodności z nowszymi
danymi. Zaktualizuj i zweryfikuj narzędzia odczytujące wynik razem z kolektorem,
zanim użyjesz nowych przechwyceń do prezentacji, generowania lub planowania
opartego na kompresji.

Przypnij dokładny artefakt wydania i jego sumę kontrolną. Weryfikacja poprzedniego
wydania nie jest dowodem, że inny plik binarny daje identyczne wyniki.

## 1.5.0

Poprzednie wydanie zapewnia przechwytywanie schematu v6 dla PostgreSQL, MySQL i
SQL Server, inspekcję plików strukturalnych, lokalny zapis Blueprint i
prezentacji oraz skrypty nadawania uprawnień uwzględniające wersję. Dokładne
źródła i artefakty znajdują się przy
[tagu wydania](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0).

Informacje o zgłaszaniu problemów zawiera [SUPPORT.md](SUPPORT.md), a wskazówki
dotyczące współtworzenia — [CONTRIBUTING.md](CONTRIBUTING.md).

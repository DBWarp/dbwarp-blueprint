# Przewodnik przeglądu dla DBA

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../DBA_REVIEW_GUIDE.md).

[English](../DBA_REVIEW_GUIDE.md) | [Deutsch](../de/DBA_REVIEW_GUIDE.md) | [Français](../fr/DBA_REVIEW_GUIDE.md) | [Español](../es/DBA_REVIEW_GUIDE.md) | [Polski](DBA_REVIEW_GUIDE.md) | [日本語](../ja/DBA_REVIEW_GUIDE.md) | [简体中文](../zh/DBA_REVIEW_GUIDE.md)

Ten przewodnik jest przeznaczony dla administratorów baz danych i recenzentów bezpieczeństwa, którzy decydują, czy uruchomić `dbwarp-blueprint` w środowisku produkcyjnym lub zbliżonym do produkcyjnego.

## Model wykonania

`dbwarp-blueprint` jest lokalnym programem wiersza poleceń. W trybie na żywo otwiera jedno połączenie z bazą danych pod URI wskazanym przez użytkownika i zapisuje lokalny plik TOML. Nie komunikuje się z infrastrukturą DBWarp, interfejsami API chmury, punktami końcowymi telemetrii, serwerami licencji ani serwerami aktualizacji.

W trybie prezentacji `--from-toml` w ogóle nie łączy się z bazą danych.

## Zalecane konto

Użyj dedykowanego konta o niskich uprawnieniach, mającego dostęp do odczytu metadanych katalogu oraz, jeśli włączono kompresję poziomu 2, uprawnienie do próbkowania wierszy z tabel użytkownika.

Zalecane właściwości:

- brak uprawnień do zapisu;
- brak uprawnień DDL;
- brak roli superużytkownika lub administratora;
- dostęp do odczytu ograniczony do ocenianej bazy danych;
- hasło lub token przekazywane przez plik albo monit, bez osadzania w URI.

Dokładne uprawnienia zależą od silnika i zasad klienta. Jeżeli konto nie może odczytać niektórych widoków katalogu lub próbkować niektórych tabel, narzędzie powinno zakończyć się z czytelnym błędem albo wygenerować ograniczony Blueprint; zachowaj dziennik audytu.

Użyj uwzględniających wersję skryptów i zastrzeżeń opisanych w
[`../../sql/grants/README.md`](../../sql/grants/README.md). Po zatwierdzonym
przechwyceniu usuń dedykowane konto kolektora za pomocą odpowiedniego skryptu z
`sql/revoke/`; przed wykonaniem sprawdź dokładną bazę danych, wzorzec hosta,
rolę i docelowe loginy.

## Poziom 1: tylko katalog

Poziom 1 jest domyślny, gdy nie podano `--measure-compression`.

Odczytuje:

- wersję silnika;
- listę tabel i zanonimizowane dane wejściowe porządkowania;
- przybliżone liczby wierszy;
- rozmiary tabel i indeksów;
- rodziny typów kolumn, możliwość występowania wartości NULL oraz zaokrąglone statystyki długości, jeśli są dostępne;
- typ indeksu, unikatowość i zanonimizowane numery porządkowe kolumn;
- strukturę grafu kluczy obcych, jeśli jest dostępny;
- opcjonalny pomiar RTT po stronie klienta, chyba że ustawiono `--no-rtt-probe`.

Nie odczytuje wartości wierszy.

## Inwentarz artefaktów innych niż tabele

Od schematu v4 Blueprinty inwentaryzują obiekty inne niż tabele niezależnie od próbkowania wierszy. Domyślne `--artifact-detail summary` odczytuje katalogi obiektów, ale nie definicje, i emituje tylko ograniczone liczniki oraz klasy zewnętrznych wymagań.

`--artifact-detail graph --yes` dodaje anonimowe identyfikatory obiektów i krawędzie zależności. `--artifact-detail analyzed --yes` dodatkowo odczytuje dostępne definicje tymczasowo i emituje tylko ograniczone przedziały cech leksykalnych i złożoności. Tekst definicji, nazwy obiektów źródłowych, punkty końcowe, nazwy dostawców, podmioty zabezpieczeń, sekrety, klucze, certyfikaty, nazwy pakietów i pliki binarne nigdy nie są serializowane.

Uprawnienia do katalogów wpływają na twierdzenia o braku. Sprawdź `visibility`, `inventory_complete`, `dependencies_complete`, `catalogs_unreadable` i `families_not_inventoried`; nie uznawaj zera za dowód, gdy te pola wskazują lukę. `DBP1410W` oznacza opcjonalny katalog artefaktów, którego nie udało się odczytać.

Anonimowa topologia zależności nadal może identyfikować aplikację. Zatwierdź `graph` lub `analyzed` tylko wtedy, gdy to ryzyko jest akceptowalne. Zobacz [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Poziom 2: pomiar kompresji

Poziom 2 jest włączany wyłącznie przez jawną parę:

```bash
--measure-compression --yes
```

Poziom 2 dodatkowo odczytuje ograniczone próbki wierszy do pamięci procesu. Bajty próbki są kodowane w wewnętrznym buforze ramek wierszy, kompresowane lokalnie przez zstd na poziomie 3, podsumowywane w postaci zaokrąglonych współczynników, a następnie usuwane.

Bajty próbek:

- nie są zapisywane w `blueprint.toml`;
- nie są zapisywane w dzienniku audytu;
- nie są zapisywane w plikach tymczasowych;
- nie są wysyłane przez żadną sieć poza połączeniem z bazą danych;
- nie są przechowywane po podsumowaniu próbki.

Poziom 2 jest wartościowy, ponieważ wydajność DBWarp i koszt ruchu wychodzącego zależą od bajtów po kompresji, a nie od surowych bajtów tabeli.

## Pomiar RTT

Domyślnie po zestawieniu połączenia narzędzie wykonuje pięć zapytań `SELECT 1`. Powstaje blok `[network]` zawierający `connect_total_ms`, `query_rtt_ms_p50` oraz `query_rtt_ms_p95`.

Pomiar pomaga operatorom zrozumieć, gdzie uruchomiono narzędzie Blueprint względem źródłowej bazy danych. Nie jest to RTT sieci WAN używanej do migracji.

Wyłącz go za pomocą:

```bash
--no-rtt-probe
```

## Odczytywane pliki

Podczas działania narzędzie odczytuje tylko pliki jawnie wskazane w wierszu poleceń, takie jak pliki haseł, pliki użytkownika, pliki CA/certyfikatów/kluczy TLS, pliki tokenów Entra albo plik wejściowy `--from-toml`.

Celowo nie odczytuje typowych niejawnych lokalizacji poświadczeń, takich jak `~/.pgpass`, `~/.my.cnf`, pliki poświadczeń chmurowych, klucze SSH, historia powłoki ani domyślne zmienne środowiskowe haseł.

Pełna lista znajduje się w [`AUDIT.md`](AUDIT.md).

## Zapisywane pliki

Narzędzie zapisuje wyłącznie w ścieżkach wybranych przez aktywny tryb:

- plik TOML Blueprint wskazany przez `--out` w trybie na żywo;
- plik `--deck`, jeśli go zażądano;
- plik `--audit-log`, jeśli go zażądano;
- w trybie wsadowym katalog `--out-dir`: `bundle.toml`, `blueprints/`, `audits/`,
  znacznik własności oraz `errors.txt`, gdy trzeba zgłosić częściowe niepowodzenie;
- dziennik audytu na stderr przy każdym uruchomieniu.

Nie używa niejawnego katalogu tymczasowego systemu operacyjnego. Atomowa
publikacja wsadowa może utworzyć obok `--out-dir` sąsiedni katalog przejściowy
lub katalog odzyskiwania; w przypadku obsłużonego błędu katalog ten jest usuwany
albo przywracany jest poprzedni pakiet.

## Lista kontrolna przeglądu danych wyjściowych

Przed udostępnieniem `blueprint.toml` sprawdź:

- nagłówek jest stałym nagłówkiem `dbwarp-blueprint v6`;
- identyfikatory tabel mają postać `table-001`;
- identyfikatory kolumn mają postać `col-1`;
- identyfikatory schematów mają postać `schema-A`;
- nie występują rzeczywiste nazwy tabel, kolumn, indeksów, schematów ani użytkowników;
- nie ma nazw obiektów innych niż tabele, tekstu definicji, ciągów punktów końcowych, poświadczeń, materiału kluczy/certyfikatów, nazw pakietów ani plików binarnych;
- nie występują wartości wierszy;
- wartości liczbowe są zaokrąglane zgodnie z opisem w [`FORMAT.md`](FORMAT.md);
- opcjonalne sekcje kompresji zawierają tylko współczynniki i metadane próbek.
- pola kompletności artefaktów ujawniają filtrowaną widoczność, nieczytelne katalogi i znane niezamodelowane rodziny.

Domyślne zrównoważone dane wyjściowe MySQL zawierają dokładne zadeklarowane
pojemności i długości prefiksów indeksów oraz względnie zaokrąglone próbki
średniej/p95. Jawnie sprawdź trzy znaczniki wierności. Jeżeli użyto
`--length-fidelity exact --yes`, zatwierdź również dokładne statystyki próbek.
Wartości wierszy i rzeczywiste nazwy obiektów nadal nie mogą występować. Brak
znaczników wierności oznacza dane starsze lub nieznane i nie może być uznawany
za metadane gotowe do benchmarku.

Znacznik nie stwierdza, że próbkowanie objęło każdą tabelę. Pakiet przekazywany
do benchmarku musi również wykazać w manifeście estymatora brak niepróbkowanych
kolumn indeksowanych o zmiennej szerokości; jeśli ta bramka zawiedzie, zwiększ
`--max-wall-secs` i ponownie wykonaj przechwycenie.

## Bezpieczeństwo operacyjne

Zalecane pierwsze uruchomienie:

```bash
--sample-rows 500 --max-wall-secs 120
```

Zalecane uruchomienie w stylu produkcyjnym po zatwierdzeniu:

```bash
--sample-rows 1000 --max-wall-secs 300
```

Uruchom narzędzie na replice do odczytu, jeżeli zasady produkcyjne zabraniają próbkowania na serwerze głównym.

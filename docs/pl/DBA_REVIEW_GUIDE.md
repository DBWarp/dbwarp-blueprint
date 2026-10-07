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
- brak uprawnień DDL, chyba że przegląd jawnie zatwierdzi rozszerzone
  przechwytywanie MySQL, którego uprawnienia metadanych `TRIGGER` i `EVENT`
  pozwalają na operacje DDL;
- brak roli superużytkownika lub administratora;
- dostęp do odczytu ograniczony do ocenianej bazy danych;
- hasło lub token przekazywane przez plik albo monit, bez osadzania w URI.

Dokładne uprawnienia zależą od używanego silnika i polityki. Jeśli konto nie ma możliwości odczytu niektórych widoków katalogu lub pobrania próbek z niektórych tabel, narzędzie wyraźnie się nie powiedzie lub wygeneruje skrócony Blueprint; należy zachować dziennik audytu.

Użyj uwzględniających wersję skryptów i zastrzeżeń opisanych w
[`../../sql/grants/README.md`](../../sql/grants/README.md). Po zatwierdzonym
przechwyceniu usuń dedykowane konto kolektora za pomocą odpowiedniego skryptu z
`sql/revoke/`; przed wykonaniem sprawdź dokładną bazę danych, wzorzec hosta,
rolę i docelowe loginy.

## Poziom 1: tylko metadane (bez próbkowania wierszy)

Poziom 1 jest domyślny, gdy nie podano `--measure-compression`.

Odczytuje:

- wersję silnika;
- listę tabel i zanonimizowane dane wejściowe porządkowania;
- przybliżone liczby wierszy;
- rozmiary tabel i indeksów;
- rodziny typów kolumn, możliwość występowania wartości NULL oraz zaokrąglone statystyki długości, jeśli są dostępne;
- typ indeksu, unikatowość i zanonimizowane numery porządkowe kolumn;
- strukturę grafu kluczy obcych, jeśli jest dostępny;
- przybliżone przedziały pojemności źródła zwracane w miarę możliwości przez punkt końcowy bazy danych;
- ograniczone liczniki obiektów innych niż tabele i wymagań zewnętrznych z
  katalogów obiektów przy domyślnym `--artifact-detail summary` (bez definicji);
- opcjonalne sondowanie RTT, chyba że ustawiono `--no-rtt-probe`.

Nie odczytuje wartości wierszy.

## Środowisko źródłowe

Blok `[source_environment]` schematu v7 jest wyprowadzany wyłącznie z wartości
zwracanych przez wybrane połączenie z bazą danych. Kolektor nigdy nie sprawdza
własnego hosta i nie przedstawia tej stacji roboczej jako serwera bazy danych.

PostgreSQL i MySQL udostępniają ustawienie bufora bazy danych przy standardowych minimalnych uprawnieniach, dlatego pamięć stanowi częściowy dowód z podstawą `database-buffer-cache`, a procesor pozostaje nieznany.

SQL Server wymaga dostępu do zasobów środowiska źródłowego tylko w trybach `--artifact-detail graph` lub `analyzed`, czyli w wersjach rozszerzonych. Tryby podstawowy i standardowy nie wysyłają zapytania o dostępność zasobów systemu operacyjnego i rejestrują zakresy dostępnych zasobów jako `not-requested`.

Ulepszony skrypt przyznaje wymagane uprawnienia na poziomie całego serwera, czyli `VIEW SERVER STATE` (2019) lub `VIEW SERVER PERFORMANCE STATE` (2022/2025), w oddzielnej partii, którą administrator bazy danych (DBA) może usunąć. Jeśli ulepsowany mechanizm przechwytywania nie może odczytać DMV, przechwytywanie kontynuuje i rejestruje katalog jako nieczytelny, zamiast pobierać wartości z lokalnej maszyny lub generować sztuczne dane dotyczące pojemności.

Ta ścieżka przechwytywania nie kontaktuje się z żadnym API chmury, Kubernetes,
hiperwizora ani systemu operacyjnego.

## Inwentarz artefaktów innych niż tabele

Blueprints niezależnie od próbkowania wierszy, katalogują obiekty, które nie są tabelami. Domyślnie `--artifact-detail summary` odczytuje katalogi obiektów, ale nie definicje, i generuje tylko ograniczone liczby oraz klasy zależności zewnętrznych.

`--artifact-detail graph --yes` dodaje anonimowe identyfikatory obiektów i krawędzie zależności. `--artifact-detail analyzed --yes` dodatkowo odczytuje dostępne definicje tymczasowo i emituje tylko ograniczone przedziały cech leksykalnych i złożoności. Tekst definicji, nazwy obiektów źródłowych, punkty końcowe, nazwy dostawców, podmioty zabezpieczeń, sekrety, klucze, certyfikaty, nazwy pakietów i pliki binarne nigdy nie są serializowane.

Uprawnienia do katalogów wpływają na twierdzenia o braku. Sprawdź `visibility`,
`inventory_complete`, `dependencies_complete`, `requirements_complete`,
`catalogs_unreadable` i `families_not_inventoried`; nie uznawaj zera ani pustej
listy wymagań za dowód, gdy te pola wskazują lukę. Przy szczegółowości
graph/analyzed sprawdź też `requirement_status` każdego obiektu: tylko
`complete` sprawia, że pusta lista dowodzi braku wymagań dla tego obiektu.
`partial` zachowuje znane fakty bez twierdzenia o pełnym pokryciu;
`unavailable` oznacza, że nie ustalono użytecznego pokrycia. W obu przypadkach
ocena powiązań wynikająca z wymagań tego obiektu pozostaje nieznana. `DBP1410W`
oznacza opcjonalny katalog artefaktów, którego nie udało się odczytać.

Anonimowa topologia zależności nadal może identyfikować aplikację. Zatwierdź `graph` lub `analyzed` tylko wtedy, gdy to ryzyko jest akceptowalne. Zobacz [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Poziom 2: pomiar kompresji

Poziom 2 jest włączany wyłącznie przez jawną parę:

```bash
--measure-compression --yes
```

Warstwa 2 dodatkowo odczytuje ograniczone próbki wierszy do pamięci procesu. Próbkowane bajty są kodowane do bufora w pamięci i wykorzystywane do obliczenia zagregowanych wartości kompresji, gęstości wartości null, cardinality/frequency, długości i stylu, zanim wartości i tymczasowe odciski cyfrowe zostaną usunięte.

Bajty próbek:

- nie są zapisywane w `blueprint.toml`;
- nie są zapisywane w dzienniku audytu;
- nie są zapisywane w plikach tymczasowych;
- nie są wysyłane przez żadną sieć poza połączeniem z bazą danych;
- nie są przechowywane po podsumowaniu próbki.

Warstwa 2 jest cenna, ponieważ czas transferu i koszty przesyłania danych zależą od skompresowanych bajtów, a nie od surowych bajtów tabeli.

## Pomiar RTT

Domyślnie po zestawieniu połączenia narzędzie wykonuje pięć zapytań `SELECT 1`. Powstaje blok `[network]` zawierający `connect_total_ms`, `query_rtt_ms_p50` oraz `query_rtt_ms_p95`.

Pomiar pomaga operatorom zrozumieć, gdzie uruchomiono narzędzie Blueprint względem źródłowej bazy danych. Nie jest to RTT sieci WAN używanej do migracji.

Wyłącz go za pomocą:

```bash
--no-rtt-probe
```

## Odczytywane pliki

Podczas działania narzędzie odczytuje tylko pliki jawnie wybrane w wierszu
poleceń albo wskazane przez jawnie wybrany manifest wsadowy lub pakiet. Mogą to
być pliki haseł, użytkowników i kluczy anonimizacji, pliki
CA/certyfikatów/kluczy TLS, pliki tokenów Entra, wejścia plików strukturalnych
oraz wejścia Blueprint lub pakietu.

Celowo nie odczytuje typowych niejawnych lokalizacji poświadczeń, takich jak `~/.pgpass`, `~/.my.cnf`, pliki poświadczeń chmurowych, klucze SSH, historia powłoki ani domyślne zmienne środowiskowe haseł.

To stwierdzenie dotyczy wykrywania poświadczeń kontrolowanego przez aplikację.
Biblioteki bazy danych, TLS, DNS i zintegrowanego uwierzytelniania mogą
korzystać z magazynów zaufania, konfiguracji i pamięci podręcznych poświadczeń
systemu operacyjnego. Gdy wymaga tego polityka hosta, sprawdź lub prześledź te
zależności platformowe oddzielnie.

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

- nagłówek jest stałym nagłówkiem `dbwarp-blueprint v7`;
- identyfikatory tabel mają postać `table-001`;
- identyfikatory kolumn mają postać `col-1`;
- identyfikatory schematów mają postać `schema-A`;
- nie występują rzeczywiste nazwy tabel, kolumn, indeksów, schematów ani użytkowników;
- nie ma nazw obiektów innych niż tabele, tekstu definicji, ciągów punktów końcowych, poświadczeń, materiału kluczy/certyfikatów, nazw pakietów ani plików binarnych;
- nie występują wartości wierszy;
- wartości liczbowe używają dokładnej lub zaokrąglonej precyzji opisanej w
  [`FORMAT.md`](FORMAT.md); dokładne pola opcjonalne traktuj jako bardziej wrażliwe;
- opcjonalne sekcje pochodzące z próbek zawierają zagregowane metadane
  kompresji, udziału NULL, kardynalności/częstotliwości, długości, stylu i
  pochodzenia próbki, nigdy próbkowane wartości.
- pola kompletności artefaktów ujawniają filtrowaną widoczność, nieczytelne katalogi i znane niezamodelowane rodziny.

Domyślny, zbalansowany wynik MySQL zawiera dokładne zadeklarowane pojemności i długości prefiksów indeksów, a także względnie zaokrąglone średnie wartości i wartości p95. Proszę sprawdzić trzy wskaźniki jakości. Jeśli użyto `--length-fidelity exact --yes`, należy również zatwierdzić dokładne statystyki pobrane w próbkach. Wartości w wierszach i rzeczywiste nazwy obiektów muszą nadal być pomijane. Plik Blueprint bez wskaźników jakości został wygenerowany przez starszą wersję; należy go ponownie utworzyć.

Oznaczenie to nie stwierdza, że próbkowanie objęło wszystkie tabele. Jeśli zostanie zgłoszony błąd `DBP1406W`, należy zwiększyć wartość `--max-wall-secs` i ponownie wykonać próbkowanie.

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

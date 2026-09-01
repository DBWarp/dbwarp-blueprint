# Pomiar kompresji

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../COMPRESSION_MEASUREMENT.md).

[English](../COMPRESSION_MEASUREMENT.md) | [Deutsch](../de/COMPRESSION_MEASUREMENT.md) | [Français](../fr/COMPRESSION_MEASUREMENT.md) | [Español](../es/COMPRESSION_MEASUREMENT.md) | [Polski](COMPRESSION_MEASUREMENT.md) | [日本語](../ja/COMPRESSION_MEASUREMENT.md) | [简体中文](../zh/COMPRESSION_MEASUREMENT.md)

`dbwarp-blueprint` może opcjonalnie zmierzyć, jak dobrze kompresują się
reprezentatywne dane tabel. Zwiększa to dokładność estymacji DBWarp, ponieważ
czas transferu WAN i koszt ruchu wychodzącego zależą od skompresowanych bajtów,
a nie od surowego rozmiaru tabeli.

Pomiar kompresji jest opcjonalny i wymaga wyraźnej zgody. Interaktywny przebieg na żywo może zaakceptować monit wstępny; przebiegi bezobsługowe i pliki strukturalne używają:

```bash
--measure-compression --yes
```

Bez tych opcji narzędzie odczytuje wyłącznie metadane katalogu.

## Co jest próbkowane

Dla każdej tabeli użytkownika narzędzie odczytuje do pamięci ograniczoną liczbę
wierszy, koduje je w deterministycznym buforze ramek wierszy, lokalnie kompresuje
ten bufor algorytmem zstd na poziomie 3, rejestruje zaokrąglone
współczynniki i odrzuca bufor.

Dla wybranych kolumn tekstowych i binarnych Poziom 2 może również próbkować
samą kolumnę. Pozwala to narzędziom planistycznym dalszego etapu dopasować
entropię poszczególnych kolumn zamiast opierać się wyłącznie na średnich na
poziomie tabeli.

Każdy pomiar to niezależna, jednoprzebiegowa ramka zstd z zadeklarowanym rozmiarem wejścia. Wariancja współczynników (`ratio_stddev`) jest mierzona na wyrównanych do wierszy fragmentach 64 KiB tego samego bufora, dzięki czemu opisuje transfer przewidywany przez estymator, a nie jedną średnią całego bufora. Ponieważ rozmiar wejścia jest deklarowany, zstd dobiera parametry zależne od rozmiaru zgodne ze sposobem modelowania transferu przez estymator. Przy małych próbkach (poniżej około 1 MiB) współczynniki mogą zauważalnie odbiegać od przechwyceń z wcześniejszych wydań mierzonych przez kontekst strumieniowy bez deklaracji rozmiaru; współczynników małych tabel nie można bezpośrednio porównywać przez tę granicę. Miarodajny jest pomiar z deklarowanym rozmiarem, bo odpowiada transferowi.

Próbkowane bajty nie są zapisywane na dysku, dołączane do `blueprint.toml` ani do
dziennika audytu i nie są nigdzie wysyłane poza przesłaniem z serwera bazy
danych do uruchomionego przez Ciebie procesu lokalnego.

## Współbieżność lokalnych workerów

Próbkowanie bazy danych zawsze korzysta z jednego połączenia sekwencyjnego.
Opcjonalne ustawienie `--compression-workers N` zrównolegla wyłącznie lokalną
kompresję odczytanych już próbek w pamięci. Przyjmuje 1–32 workerów, a domyślna
wartość 1 minimalizuje wpływ na host źródłowy. Zwiększ ją jawnie, aby użyć
większej ilości lokalnego CPU:

```bash
--measure-compression --yes \
--compression-workers 4
```

Wyższe wartości mogą skrócić czas, gdy wąskim gardłem jest zstd, lecz zwiększą
lokalne użycie CPU i szczytową pamięć. Nie tworzą równoległych połączeń
próbkujących bazę. Każdy worker ma własne konteksty zstd, a kolejka wejściowa
jest ograniczona do liczby workerów. Kolejność wyjścia i wartości Blueprint v6
pozostają deterministyczne.

Kolektor pomija zapytania o wiersze i styl tylko wtedy, gdy utrzymywana przez
silnik wartość katalogowa bezpiecznie potwierdza pustą tabelę w chwili odczytu
katalogu. PostgreSQL wymaga świeżych przeanalizowanych statystyk bez późniejszych
zmian; SQL Server używa licznika wierszy partycji. Szacunki liczby wierszy MySQL
mogą zwracać zero dla niepustej tabeli, więc kolektor nie używa ich do pomijania
próbkowania. Ta ostrożna różnica chroni wierność.

## Co pojawia się w pliku Blueprint

Emitowane są wyłącznie liczby podsumowujące. Dla kolumn podobnych do tekstu
przebieg Poziomu 2 może emitować ograniczoną etykietę stylu, taką jak `json`,
`xml`, `natural-text`, `base64`, `hex`, `numeric-text` lub `mixed`.

Przykład:

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "dbwarp-blueprint-rowframe-v1"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

Wartości te pomagają zatwierdzonym narzędziom dalszego etapu oszacować rozmiar
transferu sieciowego i generować syntetyczne dane tekstowe i binarne o podobnej
podatności na kompresję.

## Dlaczego ma to znaczenie

Dwie bazy danych o takim samym surowym rozmiarze tabel mogą zachowywać się
zupełnie inaczej podczas migracji:

- JSON, XML, powtarzające się kody biznesowe, rzadki tekst i tekst w języku
  naturalnym często dobrze się kompresują.
- Zaszyfrowane wartości, już skompresowane obiekty blob, losowe tokeny i dane
  binarne o wysokiej entropii nie kompresują się dobrze.
- Dane SQL Server `nvarchar` mają inny rozkład bajtów niż tekst UTF-8 i są
  odpowiednio kodowane na potrzeby próbkowania.

Niewielki pomiar lokalny jest zwykle bardziej użyteczny niż zgadywanie na
podstawie typów kolumn.

## Obciążenie próbki i przejrzystość

Niektóre silniki nie oferują idealnie równomiernego próbkowania tabel. Gdy
narzędzie przechodzi na mniej idealną metodę, plik Blueprint oznacza to polami
`sampled_with_bias` i `bias_reason`.

Obciążone próbki są nadal przydatne, ale narzędzia dalszego etapu powinny
traktować je z mniejszą ufnością. Dziennik audytu rejestruje, że próbkowanie
wierszy było włączone, oraz liczbę lokalnie zakodowanych bajtów row-frame.
Bajty sieciowe są oznaczone jako `unknown`, jeśli sterownik ich nie udostępnia.

## Praktyczne ustawienia próbkowania

Pierwszy przebieg bezpieczny dla środowiska produkcyjnego:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Lepsze dane wejściowe estymatora, gdy dostępna jest replika do odczytu lub okno
konserwacyjne:

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

Duże bazy danych nie wymagają ogromnych próbek. Celem jest stabilny sygnał
kompresji, a nie dokładne profilowanie na poziomie wiersza. `--max-wall-secs`
jest twardym limitem całego przechwytywania na żywo, łącznie z połączeniem,
katalogami, RTT i próbkowaniem; nie jest nowym budżetem dla każdej fazy.

Próbkowanie bazy danych na żywo ma również niekonfigurowalny limit 16 MiB
projektowanego ładunku na tabelę. Projekcja SQL obcina komórki o zmiennej
szerokości po stronie serwera i zmniejsza limit wierszy dla wyjątkowo szerokich
tabel, zanim sterownik otrzyma dane. W rezultacie bardzo duże wartości LOB
wpływają przez ograniczone prefiksy, a nie pełną zawartość. Audyt zapisuje
aktywny limit ładunku tabeli i dokładną łączną liczbę bajtów ramek wierszy
zakodowanych lokalnie.

## Jak konsumenci dalszego etapu wykorzystują te dane

Konsument dalszego etapu powinien używać dowodów kompresji w następującej kolejności:

1. rozpoznane bloki kompresji poszczególnych kolumn;
2. rozpoznane bloki kompresji na poziomie tabeli;
3. wartości domyślne typu i stylu, gdy nie istnieje zmierzony współczynnik.

Pole `sample_encoding` jest częścią kontraktu. Konsumenci powinni używać tylko
współczynników z rozpoznanym znacznikiem kodowania, ponieważ różne kodowania
próbki mogą dawać różne współczynniki kompresji dla tych samych danych
logicznych.

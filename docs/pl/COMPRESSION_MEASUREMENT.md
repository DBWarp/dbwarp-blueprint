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

Gdy pomiar kompresji jest wyłączony, przechwytywanie bazy danych na żywo nie
próbkuje wartości wierszy z tabel użytkownika. Zachowanie plików strukturalnych
jest inne: rekordy Avro nadal muszą zostać odczytane sekwencyjnie w celu
zebrania liczby wierszy, długości i metadanych wartości null; zobacz
[Pliki strukturalne](STRUCTURED_FILES.md).

## Co jest próbkowane

Dla każdej kwalifikującej się tabeli użytkownika, której pustki nie da się
bezpiecznie potwierdzić, narzędzie odczytuje do pamięci ograniczoną liczbę
wierszy, koduje je w stabilnych, przejściowych buforach sondy, lokalnie kompresuje
te bufory algorytmem zstd na poziomie 3 i wyprowadza zagregowane pomiary kompresji,
udziału NULL, kardynalności/częstotliwości, długości i stylu, po czym odrzuca
próbkowane wartości i tymczasowe odciski.

Dla wybranych kolumn text/binary, Tier 2 może również pobierać próbki tylko tej kolumny. Pozwala to na określenie stopnia kompresji dla każdej kolumny, zamiast tylko średnich wartości dla całego tabeli.

Współczynniki tabel w bazach danych działających na żywo wykorzystują neutralną sekwencję ograniczonych grup po 1000 wierszy, z jednym opisem dla każdej kolumny, stałą długością wartości i sąsiadującymi danymi w kolumnach. To mierzy strukturę istotną dla kompresji, bez zbierania jakichkolwiek danych z bazy danych ani protokołu transferu. Współczynniki dla każdej kolumny zachowują wartość `blueprint-compression-probe-v2`, której oznaczona, długościowo poprzedzona wartość pozostaje bardziej szczegółowym źródłem entropii.

Bloki tabel PostgreSQL używają `blueprint-columnar-transfer-probe-v2`, które przesyłają grupy wierszy przez jeden trwały kontekst zst z poziomem 3 i zrzucają dane po każdej grupie. MySQL i SQL Server używają `blueprint-columnar-transfer-probe-v3`: te same neutralne bajty i trwały kontekst, z dodatkowymi zrzutami danych w granicach bloków o rozmiarze 256 KiB. Próbki `nvarchar`, `nchar` i `ntext` dla SQL Server są mierzone jako rozkłady bajtów UTF-16LE. Próbki `varchar`, `char` i `text` dla SQL Server zachowują swoją zmierzoną szerokość bajtów; Blueprint zapisuje kod strony katalogu źródła jako `utf-8`, `windows-N` lub `code-page-N`. Sterownik bazy danych nadal udostępnia zdekodowane ciągi próbkującemu, więc nie można twierdzić, że bajty mają identyczność dla starszych kodowań stron. Tabela `ratio_stddev` jest mierzona w oparciu o wyjściowe grupy wierszy. Bloki projekcji dla poszczególnych kolumn pozostają niezależnymi, jednorazowymi pomiarami entropii i emitują `0.0`. Blueprinty z wcześniejszych wersji mogą zawierać `blueprint-columnar-transfer-probe-v1`; współczynniki z różnymi znacznikami nie są porównywalne.

Próbkowane bajty trafiają do procesu lokalnego wyłącznie przez wybraną sesję
bazy danych. Nie są zapisywane na dysku, dołączane do `blueprint.toml` ani do
dziennika audytu, wysyłane ani przekazywane do infrastruktury DBWarp.

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
jest ograniczona do liczby workerów. Liczba workerów nie zmienia pomiarów.
Kolejność anonimowych etykiet celowo zmienia się przy domyślnym nowym kluczu;
używaj ponownie chronionego pliku `--anonymization-key-file` tylko do
zatwierdzonych porównań między uruchomieniami.

Kolektor pomija zapytania o wiersze i styl tylko wtedy, gdy utrzymywana przez
silnik wartość katalogowa bezpiecznie potwierdza pustą tabelę w chwili odczytu
katalogu. PostgreSQL wymaga świeżych przeanalizowanych statystyk bez późniejszych
zmian; SQL Server używa licznika wierszy partycji. Szacunki liczby wierszy MySQL
mogą zwracać zero dla niepustej tabeli, więc kolektor nie używa ich do pomijania
próbkowania. Ta ostrożna różnica chroni wierność.

## Co pojawia się w pliku Blueprint

Emitowane są wyłącznie zagregowane podsumowania. Dla kolumn podobnych do tekstu
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
sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "blueprint-compression-probe-v2"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "blueprint-columnar-transfer-probe-v3"
```

Te wartości są wykorzystywane do oszacowania rozmiaru przesyłanych danych przez sieć.

## Dlaczego ma to znaczenie

Dwie bazy danych o takim samym surowym rozmiarze tabel mogą zachowywać się
zupełnie inaczej podczas migracji:

- JSON, XML, powtarzające się kody biznesowe, rzadki tekst i tekst w języku
  naturalnym często dobrze się kompresują.
- Zaszyfrowane wartości, już skompresowane obiekty blob, losowe tokeny i dane
  binarne o wysokiej entropii nie kompresują się dobrze.
- Tekst Unicode i tekst wąski w SQL Server mają różne rozkłady bajtów. Próbnik
  modeluje `nvarchar` jako UTF-16LE i zapisuje stronę kodową kolacji potrzebną
  do interpretacji `varchar`, zamiast traktować każdą kolumnę tekstową jako UTF-8.

Niewielki pomiar lokalny jest zwykle bardziej użyteczny niż zgadywanie na
podstawie typów kolumn.

## Obciążenie próbki i przejrzystość

Niektóre silniki nie oferują idealnie równomiernego próbkowania tabel. MySQL
rozkłada ograniczoną próbkę na cztery zakresy numerycznego klucza podstawowego,
jeśli ta ścieżka dostępu jest dostępna; w przeciwnym razie przechodzi na
`LIMIT N`. Obie metody są jawnie oznaczone jako obciążone, ponieważ żadna nie
jest statystyczną próbką losową. Każde okno zakresu poza ostatnim ma wyłączną
górną granicę. Rzadkie lub nierównomierne obszary klucza podstawowego mogą więc
dać niepełne okno, ale żadne okno nie odczyta ponownie wierszy z następnego. Inne
mniej idealne metody awaryjne silników są również zapisywane przez
`sampled_with_bias` i `bias_reason`.

Blueprint rejestruje strukturę ograniczonej próbki oddzielnie od pól tekstowych. Pobieranie próbek zakresów kluczy głównych w MySQL generuje `sample_layout = "primary-key-range-windows"` i sortuje każdy fragment według pełnego klucza głównego.

Próbki obciążone nadal są przydatne, ale charakteryzują się niższą wiarygodnością. Dziennik audytu rejestruje, że włączono próbkowanie wierszy oraz lokalnie zakodowaną liczbę bajtów sondy. Sumy bajtów sesji bazy danych są raportowane jako `unknown`, gdy sterownik ich nie udostępnia.

## Praktyczne ustawienia próbkowania

Pierwszy przebieg bezpieczny dla środowiska produkcyjnego:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Bardziej precyzyjne pomiary, gdy dostępna jest replika odczytu lub okno konserwacji:

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
ładunku projekcji na tabelę. Początkowa projekcja SQL ma budżet zależny od typu
i osobno obserwuje pierwotne długości w oktetach. Gdy wartość projekcji została
skrócona, MySQL i SQL Server mogą ponowić próbę z mniejszą liczbą wierszy i
zmienionymi limitami kolumn, które nadal mieszczą się w budżecie. Wartości zbyt
szerokie dla tego budżetu pozostają ograniczonymi prefiksami; informacje o
pochodzeniu pomiarów kompresji i podsumowań wartości dokumentują to ograniczenie,
natomiast statystyki długości zachowują pierwotne długości próbkowanych wartości
zgłoszone przez serwer, z zastosowaniem wybranej polityki wierności długości.

Limit nie dotyczy bajtów sieciowych ani pamięci procesu. Kodowanie protokołu,
metadane pierwotnych długości, ponowienia i bufory sterownika powodują dodatkowy
narzut. Audyt zapisuje skonfigurowany limit ładunku, wykonane zapytania i dokładną
łączną liczbę lokalnie zakodowanych bajtów sondy; nie raportuje zmierzonego ruchu
na połączeniu z bazą danych.

## Jak interpretowane są wyniki pomiarów.

Pole `sample_encoding` jest częścią umowy. Współczynniki są porównywalne tylko w obrębie jednego tagu kodowania, ponieważ różne sposoby kodowania próbek mogą dawać różne współczynniki kompresji dla tych samych danych logicznych. W szczególności, współczynnik transferu kolumnowego na poziomie tabeli oraz współczynniki v2 dla poszczególnych kolumn to uzupełniające się pomiary i nie powinny być ze sobą zamieniane.

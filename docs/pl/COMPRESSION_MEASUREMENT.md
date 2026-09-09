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

Dla wybranych kolumn tekstowych i binarnych Poziom 2 może również próbkować
samą kolumnę. Pozwala to narzędziom planistycznym dalszego etapu dopasować
entropię poszczególnych kolumn zamiast opierać się wyłącznie na średnich na
poziomie tabeli.

Współczynniki tabel aktywnych baz danych korzystają z neutralnej sekwencji
ograniczonych grup po 1000 wierszy, z jednym deskryptorem na kolumnę, długościami
wartości o stałej szerokości i ładunkami ułożonymi kolumnowo. Mierzy to strukturę
istotną dla kompresji, wspólną dla transportów masowych, bez odwzorowywania
protokołu bazy ani formatu sieciowego DBWarp. Współczynniki kolumn zachowują
`blueprint-compression-probe-v2`; oznaczone wartości z prefiksem długości nadal
stanowią bardziej szczegółowe wejście entropii.

Bloki tabel PostgreSQL używają obecnie
`blueprint-columnar-transfer-probe-v2`, który przepuszcza grupy wierszy przez
jeden trwały kontekst zstd poziomu 3 i opróżnia go po każdej grupie. MySQL i SQL
Server używają `blueprint-columnar-transfer-probe-v3`: tych samych neutralnych
bajtów i trwałego kontekstu, z dodatkowymi opróżnieniami na granicach bloków
sondy o rozmiarze 256 KiB. Próbki SQL Server `nvarchar`, `nchar` i `ntext`
są mierzone jako rozkłady bajtów UTF-16LE. `varchar`, `char` i `text` zachowują
próbkowaną wąską szerokość bajtów; Blueprint zapisuje stronę kodową katalogu
kolacji źródłowej jako `utf-8`, `windows-N` lub `code-page-N`, aby zatwierdzony
konsument mógł wybrać zgodny natywny koder zamiast poszerzać wartości. Sterownik
nadal udostępnia próbnikowi zdekodowane ciągi, więc nie deklaruje się identyczności
bajtów dla starszych stron kodowych. Tabelowy `ratio_stddev` jest mierzony między
wyjściami zewnętrznych grup wierszy. Bloki projekcji kolumn pozostają niezależnymi,
jednoprzebiegowymi pomiarami entropii i emitują `0.0`. Starsze pomiary tabel
oznaczone `blueprint-columnar-transfer-probe-v1` wykonywały jedną operację z
zadeklarowanym rozmiarem na połączonych ramkach; jawna wersja zapobiega cichemu
reinterpretowaniu tych współczynników zgodnie z obecną polityką strumieniową.

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
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_empty_TABLESAMPLE"
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
jest statystyczną próbką losową. Inne mniej idealne metody awaryjne silników są
również zapisywane przez `sampled_with_bias` i `bias_reason`.

Gdy układ ograniczonej próbki wpływa na generowanie syntetyczne, Blueprint
zapisuje go niezależnie od tych pól tekstowych. Próbkowanie zakresów numerycznego
klucza podstawowego MySQL emituje
`sample_layout = "primary-key-range-windows"` i porządkuje każde okno według
pełnego klucza podstawowego. Pozwala to konsumentom zachować grupową lokalność
kluczy złożonych bez analizowania `sample_method` ani `bias_reason`.

Obciążone próbki są nadal przydatne, ale narzędzia dalszego etapu powinny
traktować je z mniejszą ufnością. Dziennik audytu rejestruje, że próbkowanie
wierszy było włączone, oraz liczbę lokalnie zakodowanych bajtów sondy.
Bajty sesji bazy są oznaczone jako `unknown`, jeśli sterownik ich nie udostępnia.

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

## Jak konsumenci dalszego etapu wykorzystują te dane

Konsument dalszego etapu powinien używać dowodów kompresji w następującej kolejności:

1. rozpoznane bloki kompresji poszczególnych kolumn;
2. rozpoznane bloki kompresji na poziomie tabeli;
3. wartości domyślne typu i stylu, gdy nie istnieje zmierzony współczynnik.

Pole `sample_encoding` jest częścią kontraktu. Konsumenci powinni używać tylko
współczynników z rozpoznanym znacznikiem kodowania, ponieważ różne kodowania
próbki mogą dawać różne współczynniki kompresji dla tych samych danych
logicznych. W szczególności tabelowy współczynnik kolumnowej sondy transferowej i
współczynniki v2 poszczególnych kolumn są pomiarami uzupełniającymi i nie wolno
ich stosować zamiennie.

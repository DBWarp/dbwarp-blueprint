# Źródła Blueprint z plików strukturalnych

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../STRUCTURED_FILES.md).

[English](../STRUCTURED_FILES.md) | [Deutsch](../de/STRUCTURED_FILES.md) | [Français](../fr/STRUCTURED_FILES.md) | [Español](../es/STRUCTURED_FILES.md) | [Polski](STRUCTURED_FILES.md) | [日本語](../ja/STRUCTURED_FILES.md) | [简体中文](../zh/STRUCTURED_FILES.md)

`dbwarp-blueprint` może utworzyć oczyszczony plik TOML Blueprint z lokalnych danych
wejściowych Parquet i Avro, gdy źródłem jest już plik, a nie działająca baza
danych.

Jest to tryb offline:

- bez połączenia z bazą danych;
- bez poświadczeń;
- bez telemetrii;
- bez zapisywania wartości wierszy w danych wyjściowych;
- identyfikatory tabel i kolumn są emitowane wyłącznie jako `table-NNN` i `col-N`;
- audyt rejestruje tylko lokalne ścieżki plików wejściowych i wyjściowych oraz
  skrót danych wyjściowych.

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

Tryb Parquet odczytuje stopkę i metadane grup wierszy. Wyprowadza:

- liczbę wierszy z metadanych pliku;
- etykiety typów kolumn z fizycznych i logicznych typów Parquet;
- dopuszczalność wartości null z poziomów definicji;
- zaobserwowany udział wartości NULL, gdy dostępne są kompletne statystyki kolumn;
- zgrubną średnią szerokość zakodowaną i współczynnik pamięci źródłowej dla każdej kolumny z metadanych fragmentów kolumn;
- liczbę bajtów obiektu źródłowego, grup wierszy i partycji oraz pochodzenie kodeka.

Przechwytywanie Parquet wyłącznie z metadanych nie wymyśla zdekodowanej szerokości p95. Opcjonalne próbkowanie z dekodowaniem zastępuje wskazówki dotyczące zakodowanej szerokości zaobserwowanymi wartościami `len_avg`, `len_p95`, `null_fraction` i logicznym `table_bytes`.

Parquet bez dekodowanego próbkowania używa nieskompresowanych bajtów fragmentów
kolumn jako logicznego oszacowania `table_bytes`. Tabelowy `ratio_storage`
porównuje ten wynik z rzeczywistym rozmiarem obiektu, a kolumnowy `ratio_storage` porównuje
nieskompresowane i skompresowane bajty fragmentu. Są to sygnały planowania
plików, nie kompresji transportowej DBWarp, i nigdy nie są emitowane jako
`ratio_zstd_3`.

## Avro

```bash
dbwarp-blueprint \
  --from-avro /data/customer-sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Kontenery obiektów Avro nie udostępniają liczby wierszy w stopce podobnej do Parquet. Dlatego tryb Avro przechodzi kontener jeden raz, aby policzyć rekordy, wyprowadzić logiczne `table_bytes` i zaobserwować dla każdej kolumny `len_avg`, `len_p95` oraz `null_fraction`. Schemat zapisu dostarcza metadane typów logicznych. `storage_bytes` i `ratio_storage` opisują kontener Avro, a nie estymację transferu DBWarp. Jest to odpowiednie do planowania estymatora i syntetycznego zestawu testowego.

## Wierność typów logicznych

Przechwytywanie plików strukturalnych zachowuje ograniczone metadane logiczne potrzebne estymatorowi: precyzję i skalę dziesiętną, rodziny daty i czasu, precyzję znacznika czasu i semantykę UTC/lokalną, UUID, stałą szerokość binarną, ciągi UTF-8 oraz surowe bajty. Pola zawierające wyłącznie NULL pozostają `type = "null"`, zamiast stawać się syntetycznym tekstem.

Zagnieżdżonych liści Parquet oraz tablic, map, rekordów i unii wielu typów Avro nie można przedstawić jako jednego dokładnego skalara SQL. Blueprint zapisuje znormalizowany typ `json` oraz `source_semantics`, takie jak `"repeated-leaf"`, `"nested-json"` lub `"multi-type-union"`. Generatory dalszego etapu muszą oznaczać te wartości jako reprezentatywne obciążenie JSON, a nie twierdzić, że dokładnie odtwarzają zagnieżdżony schemat.

Rdzenie nazw plików źródłowych, ścieżki Parquet, nazwy pól Avro i etykiety batch `logical_table` nie są zapisywane jako identyfikatory Blueprint. Wieloplikowy zbiór danych emituje deterministyczne identyfikatory `table-NNN`, agreguje bajty obiektów, partycje, grupy wierszy, kodeki, szerokości, udziały NULL oraz zgodne pochodzenie kompresji i odrzuca pliki, których logiczne kontrakty kolumn są różne.

## Próbkowanie kompresji zdekodowanych danych

Tryb plików strukturalnych obsługuje opcjonalne próbkowanie kompresji
zdekodowanych danych:

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Te same opcje działają z `--from-avro`.

Po włączeniu `dbwarp-blueprint`:

- dekoduje do `--sample-rows` rekordów z pliku;
- koduje próbkowane wartości za pomocą tej samej ramki wiersza
  `dbwarp-blueprint-rowframe-v1`, której używa przechwytywanie Blueprint działającej
  bazy danych;
- emituje podsumowania kompresji zstd-3 na poziomie tabeli i kolumn;
- zapisuje `sample_encoding = "dbwarp-blueprint-rowframe-v1"` w wygenerowanym pliku
  TOML;
- przechowuje próbkowane bajty wyłącznie w pamięci i nigdy nie zapisuje wartości
  wierszy na dysku.

`--measure-compression` wymaga `--yes`, ponieważ odczytuje zdekodowane wartości
klienta, mimo że utrwala tylko zagregowane współczynniki.

Obecny próbnik używa deterministycznej próbki pierwszych N elementów. Jest to
powtarzalne i niedrogie, ale może być obciążone, jeśli plik jest posortowany lub
zgrupowany. W przypadku estymacji o wysokiej wadze wybierz reprezentatywny plik
albo wygeneruj wiele plików Blueprint z różnych fragmentów. Przyszła wersja może
dodać próbkowanie warstwowe według grup wierszy lub bloków.

## Zakres

Tryb Blueprint plików strukturalnych jest przydatny do:

- wymiarowania importu Parquet/Avro przed uruchomieniem DBWarp;
- generowania neutralnego wobec klienta syntetycznego zestawu testowego z
  metadanych pliku;
- planowania przepływów Parquet/Avro -> DBWarp columnar -> docelowa baza danych.

Nie zastępuje przechwytywania Blueprint działającej bazy danych, gdy rzeczywistym
źródłem jest obsługiwana baza danych, czyli PostgreSQL, MySQL lub SQL Server.
Katalog bazy danych zawiera szczegóły indeksów, kluczy, kluczy obcych,
aktualności statystyk i układu silnika, których nie ma w ogólnych metadanych
plików.

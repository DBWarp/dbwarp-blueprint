# Format pliku dbwarp-blueprint v6

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../FORMAT.md).

[English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | [Français](../fr/FORMAT.md) | [Español](../es/FORMAT.md) | [Polski](FORMAT.md) | [日本語](../ja/FORMAT.md) | [简体中文](../zh/FORMAT.md)

Czytelny dla człowieka. Łatwy do porównywania. Możliwy do analizy
kryminalistycznej.

> **Ten format ogranicza ryzyko ukrytych kanałów i bezpośredniego ujawnienia
> dzięki ograniczonemu schematowi, deterministycznym identyfikatorom i
> udokumentowanej precyzji liczb. Anonimowa struktura grafu i dokładne pola
> opcjonalne nadal mogą identyfikować obciążenie, dlatego sprawdź plik zgodnie
> z własną polityką klasyfikacji danych.**

## Nagłówek pliku

Dosłownie, bajt po bajcie:

```
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

Pusty wiersz jest częścią kontraktu. Narzędzie emituje dokładnie ten nagłówek i
żadne inne komentarze. Ułatwia to wykrywanie nieoczekiwanej treści komentarzy;
nie oznacza jednak, że pozostałe pola strukturalne nie mogą identyfikować
charakterystycznego schematu lub grafu zależności.

## Pola najwyższego poziomu

| Pole | Typ | Opis |
|---|---|---|
| `schema_version` | int | Wersja formatu. Obecnie `6`; wersje 1–5 pozostają czytelne. |
| `generated_at` | ISO-8601 string | Znacznik czasu UTC, dokładność do sekund, bez części ułamkowej. **Możliwy do przypięcia** za pomocą opcji CLI `--generated-at "2026-04-26T00:00:00Z"` dla przebiegów powtarzalności dających identyczne bajty. Dziennik audytu zapisuje `generated_at_pin: ...` zawsze, gdy opcja jest ustawiona, aby przypięcie było widoczne w śladzie audytowym. Ta opcja jest jedynym sposobem przypięcia wartości — żadna zmienna środowiskowa nie jest nigdy odczytywana, zgodnie z kontraktem zaufania README „no env vars read by default”. |
| `engine` | string | `"postgresql"`, `"mysql"` lub `"sqlserver"`. |
| `engine_version` | string | Ciąg wersji zwrócony przez silnik bazy danych. |
| `source_kind` | string | Jedna z wartości `"production"`, `"staging"`, `"scrubbed-replica"`, `"synthetic"`. Deklarowana przez klienta. |
| `length_metadata` | string | Starszy znacznik zgodności: `"hybrid-v2"`, `"exact"`, `"rounded"` lub `"not-captured"`. Nowi konsumenci muszą używać trzech poniższych pól. |
| `declared_length_fidelity` | string | `"exact"` dla zadeklarowanych pojemności znakowych PostgreSQL oraz domyślnego zrównoważonego i dokładnego trybu MySQL; `"coarse-rounded-v1"` dla ścisłej prywatności MySQL; `"not-captured"`, gdy niedostępne. |
| `index_length_fidelity` | string | `"exact"` dla domyślnych zrównoważonych/dokładnych prefiksów indeksów MySQL; `"rounded-down-v1"` dla ścisłej prywatności; `"not-captured"`, gdy niedostępne. |
| `observed_length_fidelity` | string | Domyślnie `"relative-rounded-v2"`, gdy wykonano próbkowanie, `"exact"` w trybie dokładnym, `"coarse-rounded-v1"` w trybie ścisłym albo `"not-sampled"`. Pokrycie próbkowaniem pozostaje oddzielnym wymaganiem dla każdej kolumny. |
| `[totals]` | inline table | Zagregowane liczby (zobacz niżej). |
| `[network]` | table | Opcjonalny dowód połączenia klient-baza i RTT zapytań. |
| `[database_topology]` | table | Wymagane dla źródeł bazodanowych schematu v6. Bezpieczne dla prywatności dane o wdrożeniu, roli lokalnej, widoczności i katalogach. Nie występuje dla plików strukturalnych. |
| `[dataset_scope]` | table | Wymagane w każdym Blueprint schematu v6. Określa zakres sum oraz kompletność pokrycia tabel, wierszy i bajtów. |
| `[tables.X]` | tables | Jedna dla każdej tabeli, zanonimizowany identyfikator. |
| `[fk_edges]` | inline table | Graf kluczy obcych między zanonimizowanymi tabelami. Opcjonalny. |
| `[artifact_inventory]` | table | Bezpieczne dla prywatności liczniki obiektów innych niż tabele, opcjonalny anonimowy graf zależności, wymagania zewnętrzne i opcjonalny ograniczony spis języka. Tylko źródła bazodanowe. |

## `[totals]`

| Pole | Typ | Dokładność |
|---|---|---|
| `table_count` | int | exact |
| `row_count` | int | suma zaokrąglonych wartości `rows` poszczególnych tabel |
| `table_bytes` | int | suma zaokrąglonych wartości `table_bytes` poszczególnych tabel |
| `index_bytes` | int | suma zaokrąglonych wartości `index_bytes` poszczególnych tabel |

Te liczby nie są automatycznie sumami dla całego klastra. Zawsze należy je
interpretować razem z `[dataset_scope]`. Brama lub koordynator systemu
shardowanego może pokazywać pozornie pełny katalog, nie przechowując bazowych
fragmentów. Schemat v6 jawnie przedstawia tę niepewność zamiast traktować
lokalne statystyki katalogowe jako globalną prawdę.

## `[database_topology]` (źródła bazodanowe schematu v6)

Ten blok zapisuje wyłącznie ograniczone fakty widoczne przez połączony endpoint
bazy danych. Nigdy nie zapisuje nazw węzłów lub hostów, adresów IP, nazw
klastrów lub kanałów replikacji, identyfikatorów serwerów ani endpointów.

| Pole | Wartości / reguła |
|---|---|
| `contract` | Zawsze `dbwarp-blueprint-topology/v1`. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` albo `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member` albo `unknown`. |
| `visibility` | `full`, `partial` albo `unknown`; opisuje dowód topologii, nie poprawność danych. |
| `member_count` | Liczba członków widocznych przez udane zapytania dowodowe. `0` oznacza nieznaną liczbę, nigdy brak członków. |
| `identifiers_redacted` | Musi mieć wartość `true`. |
| `role_counts` | Opcjonalne liczniki według zamkniętego tokenu roli. Pełna widoczność wymaga, aby ich suma równała się `member_count`. |
| `features` | Posortowane zamknięte tokeny, takie jak `citus`, `mysql-group-replication`, `mysql-galera`, `mysql-ndb`, `postgresql-streaming-replication`, `sqlserver-availability-group` lub `vitess`. |
| `catalogs_read` | Posortowane zamknięte etykiety poprawnie odczytanych katalogów topologii. |
| `catalogs_unreadable` | Posortowane zamknięte etykiety nieczytelnych katalogów topologii. Każdy wpis wyklucza deklarację pełnej widoczności. |

Zwykły endpoint może prawidłowo zgłaszać `deployment = "unknown"`, a
jednocześnie dostarczać kompletne lokalne statystyki pełnej kopii. Blueprint
nie zakłada, że zwykły serwer jest `single-node` tylko dlatego, że nie wykryto
funkcji klastrowej.

## `[dataset_scope]` (schemat v6)

Ten blok niezależnie kwalifikuje każdą sumę rozmiarową. Konsumenci muszą
odrzucić niekwalifikowaną arytmetykę całego zbioru, gdy dowolny wymagany wymiar
kompletności ma wartość `incomplete` albo `unknown`.

| Pole | Wartości / reguła |
|---|---|
| `contract` | Zawsze `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` albo `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` albo `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` albo `unknown`. |
| `size_completeness` | `complete`, `incomplete` albo `unknown`. |
| `row_count_method` | Zamknięty token pochodzenia, np. `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter` lub `distributed-aggregate`. |
| `size_method` | Zamknięty token pochodzenia, np. `postgres-local-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `citus-distributed-relation-size` lub `distributed-aggregate`. |
| `limitations` | Posortowane zamknięte przyczyny niepełnego lub nieznanego pokrycia. Co najmniej jedna jest wymagana, chyba że wszystkie wymiary są kompletne. |

`selection-limited` oznacza, że sumy i deklaracje kompletności obejmują dokładnie schematy wskazane powtarzalnym selektorem trybu na żywo `--schema`; nie deklarują pokrycia całej połączonej bazy danych. Pominięcie `--schema` zachowuje przechwytywanie wszystkich widocznych schematów.

Natywne kolektory PostgreSQL, MySQL i SQL Server sprawdzają obsługiwane
katalogi topologii przed uznaniem lokalnych statystyk za reprezentatywne dla
logicznego zbioru. Znane bramy rozproszone wyłączają niebezpieczne sumy, gdy
brakuje wiarygodnego agregatu. Awaryjny formater SQL nie ma sondy topologii,
więc emituje użyteczne oszacowania lokalne ze wszystkimi wymiarami oznaczonymi
jako `unknown` oraz ograniczeniami `topology-unobserved` i
`topology-visibility-unknown`.

Strukturalne Blueprinty Parquet i Avro pomijają `[database_topology]` i używają
`layout = "structured-dataset"` z pochodzeniem stopki lub kontenera.

Blueprint nie uruchamia testu szybkości pamięci masowej podczas zwykłego
zbierania ani nie wnioskuje o sprzęcie serwera bazy z maszyny klienta. Sumy
bajtów opisują rozmiar danych według nazwanej metody katalogowej; nie deklarują
typu dysku, IOPS, przepustowości, CPU, RAM ani wydajności migracji docelowej.

## `[network]` (opcjonalne)

Statystyki czasu obiegu sieciowego zaobserwowane po stronie klienta od narzędzia
Blueprint do źródłowej bazy danych. **NIE** jest to RTT między źródłem i celem
migracji — jest to jedynie dowód odległości narzędzia Blueprint od źródłowej bazy
danych klienta w czasie uruchomienia. Estymator działający na dalszym etapie używa go tylko
do kontroli wiarygodności RTT migracji podanego przez operatora (np. deklarowane
przez operatora 200 ms RTT migracji jest niewiarygodne, jeśli lokalna sonda
klienta wskazała 0,4 ms — narzędzie Blueprint prawdopodobnie działało na samej
źródłowej bazie danych).

Sonda działa po ustanowieniu połączenia, a przed zapytaniami katalogowymi, więc
rozgrzewanie pamięci podręcznej zapytań nie zniekształca pomiarów. Wykonuje
**5× `SELECT 1`** i emituje medianę opóźnienia. Każde `SELECT 1` zwraca stałą
liczbę całkowitą 1 — sonda nigdy nie odczytuje danych wierszy.

Blok jest nieobecny, gdy klient przekazał `--no-rtt-probe` albo gdy sama sonda
zakończyła się niepowodzeniem w trakcie działania (zapisanym jako niekrytyczne
ostrzeżenie do stderr i dziennika audytu; plik Blueprint nadal jest emitowany bez
tego bloku).

| Pole | Typ | Dokładność |
|---|---|---|
| `sample_count` | int | exact (zawsze 5 w v1) |
| `connect_total_ms` | int | Całkowity czas zegarowy od rozpoczęcia połączenia TCP do gotowej uwierzytelnionej sesji, w milisekundach. Obejmuje uzgadnianie TCP + uzgadnianie TLS (gdy ma zastosowanie) + wyzwanie/odpowiedź uwierzytelniania. Zaokrąglany do najbliższej ms. Zwykle 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | Mediana opóźnienia pojedynczego obiegu dla 5 próbek `SELECT 1`, w milisekundach. Zaokrąglana do najbliższej ms. Naturalny poziom szumu sieciowego (w praktyce ≥ 1 ms) jest szerszy niż dokładność zaokrąglenia, co usuwa ukryty kanał w najmniej znaczących bitach bez utraty użytecznej dokładności. Wartości LAN poniżej ms zapadają się do 0 lub 1. |
| `query_rtt_ms_p95` | int | 95. percentyl dla 5 próbek, obliczony metodą najbliższej rangi (najwolniejsza obserwacja), w milisekundach. Zaokrąglany do najbliższej ms. Wraz z p50 pomaga wykryć krótkie skoki opóźnienia; pięć próbek służy jedynie do orientacji i nie stanowi benchmarku obciążenia roboczego. |

Pięć zapytań sondy pojawia się w dzienniku audytu jako **pojedynczy wpis
podsumowujący** (a nie 5 oddzielnych wierszy), oznaczony
`5x SELECT 1 (RTT probe; constant integer 1, no row data)` — zgodnie z zasadą
zaufania, że zawartość wierszy nie jest odczytywana.

## `[tables.<id>]`

Identyfikatorem jest `table-NNN`, gdzie `NNN` to indeksowany od 1 numer porządkowy
w kolejności HMAC-SHA256 nazw schematu i tabeli z separacją domen. Domyślny klucz
jest generowany na nowo dla procesu i nigdy nie jest emitowany. Przekazanie tego
samego przechowywanego przez klienta `--anonymization-key-file` zachowuje
kolejność między zatwierdzonymi uruchomieniami porównawczymi.

| Pole | Typ | Dokładność / wartości |
|---|---|---|
| `rows` | int | rounded: najbliższe 100 (≤10k), 1000 (≤1M), 10000 (>1M) |
| `table_bytes` | int | rounded: najbliższe 1KiB / 1MiB / 100MiB zależnie od wielkości |
| `index_bytes` | int | rounded: tak samo jak `table_bytes` |
| `schema` | string | zanonimizowany identyfikator `schema-A`, `schema-B`, ..., `schema-AA` |
| `kind` | string | Opcjonalny zamknięty token schematu v6: `partitioned`, `materialized-view`, `temporal-current`, `temporal-history`, `memory-optimized`, `external`, `graph-node` albo `graph-edge`. Pomijany dla zwykłej tabeli lub nieznanych danych. |
| `unlogged` | bool | Opcjonalna obserwacja katalogu PostgreSQL w schemacie v6. Pomijana, gdy jej nie zebrano; jawne `false` potwierdza tabelę rejestrowaną. |
| `partition_strategy` | string | Opcjonalny token schematu v6 dla `partitioned`: `range`, `list`, `hash`, `key` albo `linear-hash`. |
| `partition_count` | int | Dokładna dodatnia liczba partycji liściowych w schemacie v6, wymagana przy `kind = "partitioned"`. |
| `partition_key_cols` | array of int | Numery kolumn prostego klucza partycjonowania w schemacie v6. Pomijane dla klucza wyrażeniowego lub braku danych katalogowych; wyrażenie nigdy nie jest serializowane. |
| `partition_rows_max` | int | Opcjonalne zaokrąglone oszacowanie wierszy największej partycji liściowej w schemacie v6. |
| `temporal_history` | string | Identyfikator powiązanej tabeli `temporal-history` w schemacie v6, wymagany dla `temporal-current`. |
| `counted_in_totals` | bool | Schemat v6. Pominięcie uwzględnia tabelę we wszystkich sumach. `external` wymaga `false`, co wyklucza ją z `table_count`, `row_count`, `table_bytes` i `index_bytes`; żadna inna jawna wartość nie jest kanoniczna. |
| `check_count` | int | Opcjonalna dokładna strukturalna liczba ograniczeń CHECK w schemacie v6. Pominięcie oznacza wartość nieznaną; `0` potwierdza ich brak. |
| `has_clustered_index` | bool | zawsze `false` dla PostgreSQL |
| `stats_freshness` | string | `"fresh"` / `"stale"` / `"never_analyzed"` (PG) — puste przy ścieżce awaryjnej SQL |
| `[tables.<id>.cols.<cid>]` | sub-tables | jedna dla każdej kolumny |
| `[tables.<id>.idxs.<iid>]` | sub-tables | jedna dla każdego indeksu |
| `[tables.<id>.compression]` | sub-table | tylko przy Poziomie 2 |

## `[tables.<id>.cols.<cid>]`

Identyfikatorem jest `col-N`, gdzie `N` to naturalna kolejność atrybutu kolumny
(indeksowana od 1 i zachowująca numer porządkowy na dysku). Stabilny między
przebiegami.

| Pole | Typ | Uwagi |
|---|---|---|
| `ordinal` | int | to samo N co w identyfikatorze |
| `type` | string | Znormalizowana rodzina typów, taka jak `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` lub `"user-defined"`. Rzeczywiste nazwy domen, typów wyliczeniowych, aliasów, typów złożonych i typów zdefiniowanych przez użytkownika nie są emitowane. |
| `nullable` | bool | |
| `value_source` | string | Opcjonalny zamknięty token schematu v6: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` albo `rowversion`. Pomijany dla zwykłej wartości lub nieznanych danych. |
| `has_default` | bool | Opcjonalna obserwacja katalogu w schemacie v6. Pominięcie oznacza wartość nieznaną; jawne `false` potwierdza brak wartości domyślnej. |
| `default_kind` | string | Opcjonalna klasyfikacja `constant`, `function` albo `expression` w schemacie v6, poprawna tylko z `has_default = true`. Tekst i literały nigdy nie są serializowane. |
| `type_kind` | string | Opcjonalny zamknięty token schematu v6: `enum`, `set`, `domain`, `composite`, `array`, `range` albo `alias`. Pomijany dla typu bazowego lub nieznanych danych. |
| `member_count` | int | Dokładna dodatnia strukturalna liczba elementów w schemacie v6, wymagana tylko dla `enum` i `set`. Ich nazwy nigdy nie są serializowane. |
| `domain_has_check` | bool | Opcjonalna obserwacja CHECK domeny w schemacie v6, poprawna tylko z `type_kind = "domain"`. |
| `hidden`, `masked`, `encrypted`, `sparse` | bool | Opcjonalne obserwacje katalogu w schemacie v6. Pominięcie oznacza wartość nieznaną; jawne `false` potwierdza brak właściwości. |
| `has_check` | bool | Opcjonalna obserwacja jednokolumnowego CHECK w schemacie v6. Każde `true` jest objęte `check_count` tabeli. |
| `null_fraction` | float | Opcjonalny zaobserwowany udział wartości NULL od `0.0` do `1.0`. Tylko zaokrąglony agregat; bitmapa wartości NULL nie jest zachowywana. |
| `native_type` | string | Opcjonalny oczyszczony typ bazowy silnika, taki jak `varchar` lub `longtext`; bez identyfikatorów, elementów typu wyliczeniowego, wartości domyślnych i wyrażeń. Obecnie emitowany przez poprawione przechwytywanie MySQL. |
| `declared_max_chars` | int | Opcjonalna zadeklarowana pojemność znakowa. Dokładna dla wartości katalogowych PostgreSQL `character`/`character varying` oraz w domyślnym zrównoważonym/dokładnym trybie MySQL; zgrubnie zaokrąglana tylko z MySQL `--length-fidelity strict`. |
| `declared_max_bytes` | int | Opcjonalna zadeklarowana pojemność bajtowa. Dokładna w domyślnym zrównoważonym/dokładnym trybie MySQL; zgrubnie zaokrąglana tylko z `--length-fidelity strict`. |
| `numeric_precision`, `numeric_scale`, `datetime_precision` | int | Opcjonalna zadeklarowana przez silnik dokładność skalarna. |
| `charset`, `collation` | string | Opcjonalne oczyszczone metadane znakowe MySQL. Są to nazwy katalogowe, nigdy identyfikatory ani wartości klienta. |
| `len_avg` | int | Próbkowana średnia liczba bajtów wartości o zmiennej długości. Domyślne względne przedziały mają około 3,2% maksymalnego błędu i dokładnie zachowują wartości do 32 bajtów; dokładne z `--length-fidelity exact --yes`; zgrubnie do najbliższych 10 tylko w trybie ścisłym. 0 = stała długość albo brak pomiaru. |
| `len_p95` | int | Próbkowany 95. percentyl z tymi samymi domyślnymi względnymi przedziałami; dokładny z `--length-fidelity exact --yes`; zgrubnie do najbliższych 100 tylko w trybie ścisłym. 0 = brak pomiaru. |
| `style` | string | Tylko Poziom 2. Jedna z wartości `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"`; puste, jeśli nie sklasyfikowano. |
| `magnitude_min`, `magnitude_max` | int | Opcjonalne dziesiętne wykładniki ze znakiem w schemacie v6, ograniczające wielkość próbkowanych liczb innych niż NULL. Są emitowane z `has_negative`; dokładne wartości nigdy nie są serializowane. |
| `has_negative` | bool | Opcjonalna obserwacja znaku w schemacie v6, emitowana tylko z obiema granicami wielkości. |
| `time_span` | string | Opcjonalny próbkowany zakres daty/czasu w schemacie v6: `intraday`, `days`, `weeks`, `months`, `years` albo `decades`. |
| `time_recent_decade` | int | Dekada najnowszej próbkowanej daty/czasu w schemacie v6, emitowana tylko z `time_span` i zawsze podzielna przez 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tylko Poziom 2. Obecne dla próbkowanych kandydujących kolumn tekstowych/binarnych. Taki sam układ pól jak dla kompresji na poziomie tabeli, ale ograniczony do jednej zanonimizowanej kolumny. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Podsumowanie rozkładu próbkowanych wartości w schemacie v3. Zawiera tylko ograniczone i zaokrąglone liczności oraz częstotliwości. |

### `[tables.<id>.cols.<cid>.cardinality]` (schemat v3)

Gdy próbkowanie wierszy jest włączone, kolektor przechowuje w pamięci najwyżej 8192 tymczasowe 64-bitowe odciski na kolumnę, wyprowadza zagregowane statystyki NDV i skośności, a następnie usuwa odciski. Ani wartości, ani odciski nie są serializowane. Blok zawiera `measured`, `sample_rows`, `non_null_rows`, `observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`, `frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`, `sample_method`, `sampled_with_bias` i `bias_reason`.

Liczności i udziały są w odpowiednich miejscach zaokrąglane dla ochrony prywatności. Statystyki służą do odtwarzania gęstości duplikatów, skośności częstych wartości i skończonych domen w syntetycznych zestawach testowych; nie pozwalają odtworzyć wartości źródłowych ani ich znaczenia biznesowego.

### `[tables.<id>.cols.<cid>.compression]` (tylko Poziom 2)

Kompresja poszczególnych kolumn jest emitowana tylko dla kandydujących kolumn
tekstowych/binarnych o ograniczonej długości, gdy użyto `--measure-compression --yes`.
Pozwala narzędziom dalszego etapu generować syntetyczne dane
tekstowe/binarne o bardziej realistycznej entropii niż same współczynniki na
poziomie tabeli.

Blok ma takie same pola jak `[tables.<id>.compression]`: `measured`,
`sample_rows`, `sample_bytes`, `sample_method`, `sampled_with_bias`,
`bias_reason`, `ratio_zstd_3`, `ratio_zstd_19`, `ratio_stddev` i
`sample_encoding`.

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
sample_method = "column TABLESAMPLE SYSTEM(0.1) LIMIT N (text format)"
sampled_with_bias = false
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

Żadne próbkowane wartości kolumn nie są zapisywane w pliku Blueprint.

## `[tables.<id>.idxs.<iid>]`

Identyfikatorem jest `idx-N`, gdzie `N` to indeksowany od 1 numer porządkowy
indeksu w tabeli, posortowany według HMAC-SHA256 nazwy indeksu z separacją domen.

| Pole | Typ | Wartości |
|---|---|---|
| `type` | string | Znormalizowana rodzina metod indeksowania, taka jak `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` lub `"other"`. Nazwy metod rozszerzeń i niestandardowych nie są emitowane. |
| `primary` | bool | Opcjonalne; emitowane jako `true` dla indeksów klucza głównego. W przeciwnym razie pominięte/false. |
| `unique` | bool | |
| `cols` | array of int | numery porządkowe uczestniczących kolumn, w kolejności kolumn indeksu |
| `prefix_lengths` | array of int | Opcjonalne długości prefiksów indeksu MySQL wyrównane z `cols`; zero oznacza pełną kolumnę. Domyślnie dokładne; zaokrąglane w dół tylko z `--length-fidelity strict`. |
| `include_cols` | array of int | Opcjonalne; numery porządkowe kolumn INCLUDE niebędących kluczami, jeśli silnik źródłowy je udostępnia. |
| `expression` | bool | Opcjonalne; true, gdy istnieje materiał klucza w postaci wyrażenia/funkcji, którego nie można przedstawić jako prostych numerów porządkowych kolumn. |
| `filtered` | bool | Opcjonalne; true dla indeksów filtrowanych/częściowych. |
| `descending` | bool | Opcjonalne; true, gdy dowolna kolumna klucza jest jawnie malejąca. |
| `prefix_distinct_counts` | array of int | Szacowana w schemacie v3 liczba odrębnych krotek dla każdego prefiksu klucza od jednej do N kolumn. Zero oznacza brak danych dla danego prefiksu. |
| `cardinality_sample_method` | string | Ograniczone pochodzenie `prefix_distinct_counts`; iloczyny wywnioskowane są jawnie oznaczone i nie są przedstawiane jako bezpośrednie próbki krotek. |

## `[tables.<id>.compression]` i `[tables.<id>.cols.<cid>.compression]` (tylko Poziom 2)

Obecne tylko wtedy, gdy plik wygenerowano z `--measure-compression --yes`.
Blok na poziomie tabeli mierzy kompletny strumień próbkowanych wierszy i
pozostaje miarodajnym współczynnikiem dla estymacji transferu całej tabeli.
Bloki na poziomie kolumn są odwzorowywane z tych samych próbkowanych wierszy,
po jednej kolumnie, i służą do pomocy generatorom syntetycznych zestawów
testowych działającym na dalszym etapie w dostrajaniu entropii poszczególnych kolumn bez
dostępu do wartości klienta. Nie wyzwalają dodatkowych odczytów bazy danych.

| Pole | Typ | Dokładność |
|---|---|---|
| `measured` | bool | zawsze `true`, jeśli blok jest obecny |
| `sample_rows` | int | exact |
| `sample_bytes` | int | rozmiar bufora próbki w pamięci, **podzielony na przedziały**: najbliższe **64 KiB** poniżej 1 MiB, najbliższe **1 MiB** poniżej 1 GiB, najbliższe **100 MiB** powyżej. Bajty nigdy nie są zapisywane na dysku. Podział na przedziały usuwa ukryty kanał najmniej znaczących bitów dla każdej tabeli, który w przeciwnym razie ujawniałaby dokładna wartość `buf.len()`. |
| `sample_method` | string | właściwy dla silnika opis ograniczonego próbkowania, na przykład `"TABLESAMPLE SYSTEM(0.1) LIMIT N"`, `"LIMIT N (fallback after empty TABLESAMPLE)"` lub `"SELECT TOP N"` |
| `sampled_with_bias` | bool | true, jeśli próbka nie jest równomierna, na przykład w przypadku ścieżki awaryjnej używającej tylko LIMIT |
| `bias_reason` | string | puste, jeśli `sampled_with_bias = false`, w przeciwnym razie znacznik taki jak `"unordered_limit_after_empty_TABLESAMPLE"` |
| `ratio_zstd_3` | float | zaokrąglony do najbliższych **0,05**, zstd poziom 3 (domyślne ustawienie produkcyjne). Zmierzony na bajtach zakodowanych za pomocą `sample_encoding`. |
| `ratio_zstd_19` | float | odziedziczony współczynnik zstd poziomu 19 akceptowany ze starszych przechwyceń; narzędzie już go nie mierzy ani nie emituje |
| `ratio_stddev` | float | zaokrąglone do najbliższych **0,05**, odchylenie standardowe współczynników poziomu 3 dla wyrównanych do wierszy fragmentów próbki o rozmiarze 64 KiB. Bloki odwzorowania na poziomie kolumn obecnie emitują `0.0`, ponieważ są pomocniczymi wskazówkami entropii, a nie modelem wariancji. |
| `sample_encoding` | string | identyfikator kodowania na poziomie bajtów, w którym próbka została skompresowana przez zstd. Obecna wartość: `"dbwarp-blueprint-rowframe-v1"`. Estymator dbwarp MUSI zweryfikować ten ciąg przed użyciem współczynnika — różne kodowania dają różne współczynniki dla tych samych danych logicznych i NIE są zamienne. Starsze pliki Blueprint mogą nie zawierać tego pola; estymatory powinny używać zmierzonych współczynników tylko wtedy, gdy znacznik kodowania jest obecny i rozpoznany. |

Podczas budowania syntetycznych zestawów testowych estymator dbwarp powinien
preferować rozpoznane bloki kompresji poszczególnych kolumn, następnie przechodzić
na kompresję na poziomie tabeli, a na końcu na wartości domyślne typu/stylu.

### Kodowanie bajtowe `dbwarp-blueprint-rowframe-v1`

Próbnik Poziomu 2 łączy wiersze lub próbkowane wartości kolumn w buforze w
pamięci przy użyciu poniższego formatu, a następnie uruchamia na nim zstd na
poziomie 3. Bufor jest odrzucany; do pliku Blueprint emitowane są tylko
wynikowe zaokrąglone współczynniki.

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

Znaczniki typów są częścią kontraktu kodowania i nie zostaną przenumerowane bez
zwiększenia przyrostka do `-v2`.

| Znacznik | Nazwa | Zastosowanie |
|---|---|---|
| 0x00 | Null | SQL NULL (bez długości i ładunku) |
| 0x01 | TextUtf8 | tekst UTF-8 |
| 0x02 | TextUtf16Le | bajty UTF-16LE, głównie SQL Server `nvarchar`/`nchar`/`ntext` |
| 0x03 | TextOther | bajty w innym zestawie znaków |
| 0x04 | NumberText | dziesiętna reprezentacja tekstowa wartości numerycznych |
| 0x05 | BoolText | wartość logiczna jako tekst |
| 0x06 | TimestampText | tekst znacznika czasu ISO-8601 |
| 0x07 | DateText | tekst daty ISO-8601 |
| 0x08 | TimeText | tekst `HH:MM:SS[.fff]` |
| 0x09 | UuidText | kanoniczny 36-znakowy tekst UUID |
| 0x0F | JsonText | JSON UTF-8 |
| 0x10 | BinaryRaw | bajty `bytea`, `varbinary`, `image` lub blob |
| 0xFE | UnknownText | zapasowa reprezentacja tekstowa dostarczona przez bazę danych |

### Granice dokładności

`ratio_zstd_3` opisuje nazwane `sample_encoding`; nie jest pomiarem bajtów protokołu bazy danych ani transportu migracyjnego. Publiczny zautomatyzowany zestaw testów sprawdza deterministyczne kodowanie, ograniczone próbkowanie i serializację, lecz nie deklaruje uniwersalnego błędu procentowego dla wszystkich silników i ścieżek ekstrakcji.

Przed użyciem współczynnika do istotnej decyzji o pojemności zakwalifikuj bieżący plik binarny i wersję silnika na reprezentatywnych danych źródłowych oraz z planowanym mechanizmem ekstrakcji. Zapisz z wynikowym planem metodę porównania, rozmiar próbki, skrót pliku binarnego, wersję silnika i zaobserwowany błąd. Zależność pierwotna to `compressed_bytes ≈ sample_bytes / ratio_zstd_3` przy rozkładzie bajtów wytwarzanym przez zapisane `sample_encoding`.

## `[fk_edges]`

Opcjonalna tabela wbudowana, w której każdy klucz jest identyfikatorem `table-NNN` odwzorowanym na listę krawędzi. Schemat v3 zachowuje numery porządkowe kolumn nadrzędnych, akcje referencyjne, tryb dopasowania, odraczalność, stan walidacji/zaufania oraz opcjonalne, bezpieczne dla prywatności podsumowanie relacji. Krawędzie są sortowane według celu, a następnie listy kolumn.

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

Opcjonalny blok `statistics` zapisuje próbkowane lub wywnioskowane wartości `non_null_rows`, `distinct_parent_values`, `parent_coverage_fraction`, fanout p50/p95/p99/max oraz `orphan_rows`, wraz z polami pochodzenia i obciążenia próbki. Zweryfikowane ograniczenia źródłowe oznaczają zero sierot. Estymaty złożone wyprowadzone z próbek poszczególnych kolumn są jawnie oznaczone jako wywnioskowane. Generatory używają tych agregatów do odtwarzania pokrycia wartości NULL i fanoutu, odwzorowując każdy złożony klucz podrzędny na jedną spójną syntetyczną krotkę nadrzędną.

## `[artifact_inventory]` (od schematu v4, źródła bazodanowe)

Niezależnie wersjonowany kontrakt `dbwarp-blueprint-artifacts/v1` opisuje obiekty
inne niż tabele bez serializowania nazw źródłowych lub definicji. Nie występuje
dla plików strukturalnych ani po wybraniu `--artifact-detail none`.

Domyślne `--artifact-detail summary` emituje `object_count`,
`external_prerequisite_count`, `counts_by_kind` i
`counts_by_external_class`. `graph` dodaje anonimowy rekord obiektu dla każdego
artefaktu oraz krawędzie zależności. `analyzed` dodaje ograniczone rekordy
`dbwarp-language-feature-census/v1` wyprowadzone tymczasowo z dostępnych
definicji. `graph` i `analyzed` wymagają jawnego `--yes`, ponieważ topologia
grafu może identyfikować aplikację.

Dowody na poziomie inwentarza obejmują:

| Pole | Wartości / reguła |
|---|---|
| `detail` | `none`, `summary`, `graph` lub `analyzed` |
| `visibility` | `full`, `privilege_filtered` lub `unknown` |
| `inventory_complete` | Może być prawdziwe tylko przy pełnej widoczności, bez nieczytelnych katalogów i zadeklarowanych niezamodelowanych rodzin |
| `dependencies_complete` | Może być prawdziwe tylko wtedy, gdy modelowane katalogi zależności były czytelne |
| `analysis_complete` | Może być prawdziwe tylko dla poziomu analyzed i gdy każda wyemitowana analiza jest kompletna |
| `catalogs_read` | Zamknięte standardowe etykiety pomyślnie sprawdzonych katalogów silnika |
| `catalogs_unreadable` | Etykiety katalogów, których odczyt nie powiódł się; każdy wpis uniemożliwia deklarację kompletności |
| `families_not_inventoried` | Znane rodziny obiektów poza bieżącym kontraktem kolektora |

Identyfikatory obiektów mają formę `<kind>-NNN`, na przykład `view-001` lub
`function-002`. Rekord zawiera wyłącznie zamknięte tokeny kind, subkind i tier,
anonimowe identyfikatory schematu/rodzica, anonimowe zależności, liczbę
nierozwiązanych zależności, ograniczoną widoczność definicji/tryb bezpieczeństwa,
opcjonalne wymaganie zewnętrzne i opcjonalny spis języka. Nazwy obiektów
źródłowych, tekst SQL, podmioty zabezpieczeń, punkty końcowe, poświadczenia,
klucze, certyfikaty i pliki binarne nie są polami kontraktu.

Wymagania zewnętrzne zapisują zamkniętą `class`, zakres wdrożenia, konieczność
użycia nieprzechwyconych materiałów binarnych/sekretów/punktów końcowych oraz
ograniczoną kategorię zgodności. Ich liczba jest dowodem do planowania migracji,
a nie deklaracją, że DBWarp może je automatycznie udostępnić lub przetłumaczyć.

Rekordy spisu języka używają `analyzer_version = "lexical-v1"` oraz
`status = "partial"`. Wartości liczby, rozmiaru, zagnieżdżenia, złożoności i
obszarów nieprzejrzystych są przedziałami, a nie dokładnymi odciskami źródła.
Cechy pochodzą z zamkniętego słownika. Analizator usuwa komentarze, literały i
cytowane identyfikatory; nie jest parserem, binderem semantycznym ani gwarancją
udanego tłumaczenia.

Zobacz [Inwentarz artefaktów innych niż tabele](ARTIFACT_INVENTORY.md), aby
poznać instrukcje operacyjne i pokrycie silników.

## Ochrona przed steganografią według wektora

| Wektor | Zabezpieczenie |
|---|---|
| Kolejność identyfikatorów | HMAC-SHA256 z separacją domen i tajnym kluczem lokalnym dla procesu uniemożliwia sprawdzanie potencjalnych nazw offline. Użyj ponownie klucza przechowywanego przez klienta tylko wtedy, gdy potrzebne są stabilne etykiety między uruchomieniami. |
| Najmniej znaczące bity liczb | Statystyki są domyślnie zaokrąglane do udokumentowanej dokładności. Tryb dokładnych długości jest jawny, wymaga zgody, jest rejestrowany w dzienniku audytu i musi być traktowany jako bardziej wrażliwe metadane. |
| Znacznik czasu poniżej sekundy | Jeden znacznik czasu UTC na początku, tylko z dokładnością do sekund |
| Formatowanie TOML | Kanoniczne: klucze alfabetyczne, stałe wcięcia, bez wstawianych komentarzy |
| Losowość próbkowania | Próbkowanie używa stałych ziaren (deterministyczne `TABLESAMPLE SYSTEM` PG). Niezależnie od tego anonimizacja identyfikatorów celowo pobiera tajny klucz z systemowego CSPRNG, chyba że klient dostarczy własny. |
| Nieużywane pola | Każde pole udokumentowano powyżej; brak pól `"metadata"`/`"comment"`/`"reserved"`, które przenoszą dane o nieograniczonej długości |
| Tekst źródłowy artefaktów i materiały zewnętrzne | Definicje są tymczasowe i zerowane po ograniczonej analizie; nazwy, tekst SQL, punkty końcowe, nazwy dostawców, poświadczenia, klucze, certyfikaty, nazwy pakietów i pliki binarne nie mają serializowanego pola |

## Zgodność wersji schematu

Bieżący producent emituje wersję schematu 6. Wersje 1–5 pozostają obsługiwane
wstecznie. Plik v1/v2 nie ma bloków rozkładu, dlatego generatory używają
deterministycznych wartości zastępczych dla typu, szerokości i równomiernych
relacji oraz zgłaszają utratę wierności. Plik v3 ma metadane rozkładu, ale nie
ma inwentarza artefaktów. Plik v4 może zawierać inwentarz artefaktów, ale
poprzedza bieżące identyfikatory kontraktu Blueprint. Czytniki normalizują
starsze identyfikatory v4 na wejściu i ponownie emitują dokument z kanonicznymi
identyfikatorami Blueprint. Plik v5 poprzedza kwalifikację topologii i zakresu
zbioru danych dodaną w v6. Konsument musi odrzucić nieznaną przyszłą wersję z
jasnym komunikatem o aktualizacji, zamiast po cichu odrzucać pola.

## Dlaczego TOML, a nie JSON

- TOML czytelniej oddziela sekcje strukturalne od danych liści
  (`[tables.table-001.cols.col-2]` zamiast zagnieżdżonego JSON).
- Jest łatwiejszy do porównywania (jeden klucz na wiersz; podtabele oparte na
  identyfikatorach pozostają ciągłe).
- Klient może edytować go ręcznie, jeśli chce usunąć lub zanonimizować określone pole przed
  udostępnieniem.

JSON jest używany jako **format pośredni** w zapasowej ścieżce SQL
(`sql/blueprint.pg.sql` tworzy JSON; `blueprint_format.py` normalizuje do TOML).
Końcowym plikiem udostępnianym dbwarp jest zawsze TOML.

## Rozszerzenia pochodzenia plików strukturalnych

Wersja schematu 3 i nowsze mogą emitować poniższe ograniczone pola.

Blueprinty plików strukturalnych używają tych samych zanonimizowanych
identyfikatorów co Blueprinty baz danych: `table-NNN` w deterministycznej kolejności
wejściowej oraz `col-N` według numeru porządkowego w schemacie. Nazwy plików,
ścieżki Parquet, nazwy pól Avro i wartość `logical_table` z manifestu nie są
emitowane jako identyfikatory tabel ani kolumn.

Gdy `engine` lub `source_kind` ma wartość `"parquet"` albo `"avro"`,
`table_bytes` jest logicznym oszacowaniem rozmiaru transferu, a `storage_bytes`
rzeczywistym rozmiarem obiektu źródłowego. Parquet bez dekodowanego próbkowania
używa nieskompresowanych bajtów fragmentów kolumn jako `table_bytes`; opcjonalne
próbkowanie zastępuje je rzutowaną liczbą bajtów
`dbwarp-blueprint-rowframe-v1`. Avro wyznacza wartość z pełnego dekodowanego
przebiegu. `source_partitions`, `row_group_count` i `source_codec` opisują układ
oraz pochodzenie planowania. Zbiory wieloplikowe agregują te wartości.
`row_group_count` dotyczy Parquet, a `source_partitions` dla jednego obiektu
wejściowego wynosi `1`.

Na poziomie kolumny `null_fraction` jest obserwowaną wartością od `0.0` do
`1.0`. `length_sample_rows` i `length_sample_method` opisują sposób uzyskania
`len_avg` i `len_p95`. `source_semantics` przechowuje ograniczone fakty, takie
jak `"repeated-leaf"`, `"nested-json"` lub `"multi-type-union"`. Precyzja
dziesiętna, precyzja i semantyka UTC/lokalna znacznika czasu, UUID oraz stały
rozmiar binarny są przenoszone przez istniejące pola skalarne i `native_type`.

Na poziomie tabeli `ratio_storage` porównuje `table_bytes` z rzeczywistymi
bajtami obiektu źródłowego. Dla kolumny Parquet porównuje nieskompresowane i
skompresowane bajty fragmentu kolumny ze stopki. Są to sygnały planowania
przechowywania plików, a nie estymacje transferu DBWarp. `ratio_zstd_3` i
`ratio_zstd_19` są prawidłowymi danymi kalibracji transferu tylko wtedy, gdy
`sample_encoding` ma wartość
`"dbwarp-blueprint-rowframe-v1"`. Współczynników stopki Parquet ani kontenera Avro
nie wolno kopiować do tych pól zstd.

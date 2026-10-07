# Zbieranie wsadowe i pakiety Blueprintów

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../BATCH_AND_BUNDLES.md).

[English](../BATCH_AND_BUNDLES.md) | [Deutsch](../de/BATCH_AND_BUNDLES.md) | [Français](../fr/BATCH_AND_BUNDLES.md) | [Español](../es/BATCH_AND_BUNDLES.md) | [Polski](BATCH_AND_BUNDLES.md) | [日本語](../ja/BATCH_AND_BUNDLES.md) | [简体中文](../zh/BATCH_AND_BUNDLES.md)

`dbwarp-blueprint` obsługuje zarówno pliki Blueprint z jednego źródła, jak i katalogi
pakietów obejmujących wiele źródeł.

Użyj pojedynczego `blueprint.toml`, gdy udostępniasz jedną bazę danych, jeden podzbiór tabel, jeden plik Parquet lub jeden plik Avro. Użyj pakietu, gdy masz wiele baz danych, wiele zestawów danych w formacie strukturalnym lub chcesz uzyskać jeden pakiet przeglądowy dla całego systemu.

## Układ pakietu

Przebieg wsadowy zapisuje katalog:

```text
customer-blueprint-bundle/
  bundle.toml
  blueprints/
    erp_pg.blueprint.toml
    billing_mysql.blueprint.toml
    orders_parquet.blueprint.toml
  audits/
    erp_pg.audit.txt
    billing_mysql.audit.txt
    orders_parquet.audit.txt
```

`bundle.toml` zawiera metadane na poziomie źródła oraz względne ścieżki do
podrzędnych plików Blueprint. Jest to preferowana forma robocza, ponieważ każde
źródło pozostaje niezależnie możliwe do przeglądania, audytowania i ponownego
uruchamiania.

Aby udostępnić pakiet jako jeden plik, spakuj katalog do jednego, osadzonego pliku TOML:

```bash
dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

Postać spakowana osadza każdy podrzędny Blueprint we wpisie jego źródła.
Zachowuje identyfikatory źródeł, tagi, identyfikatory grup zbiorów danych i
metadane ścieżek audytu dostarczone przez operatora, dlatego używaj anonimowych
wartości manifestu i sprawdź spakowany plik przed przekazaniem. Katalog roboczy
jest łatwiejszy do przeglądu, ale zawiera także szczegółowe audyty i ewentualny
`errors.txt`; domyślnie nie przekazuj go w całości.

## Kontrakt pakietu

Bieżące pakiety używają `schema_version = 3` oraz
`kind = "dbwarp-blueprint-bundle"`. Pakiet katalogowy wskazuje każdy podrzędny
Blueprint za pomocą `blueprint_path`, a pakiet spakowany osadza go w polu
`blueprint`. Narzędzia zapisujące emitują wyłącznie te kanoniczne identyfikatory.

Paczka w wersji v1 lub v2 jest nadal odczytywana i jest normalizowana do wersji v3. Te wersje nie rejestrują, czy źródła są niezależne, replikami, czy fragmentami, więc ich relacja jest oznaczana jako `unknown`, a sumy zagregowane z różnych źródeł są pomijane. Ścieżki potomne muszą być względne i muszą pozostać wewnątrz katalogu paczki po kanonizacji.

Pakiet v3 oddziela fizyczne źródła przechwytywania od logicznych zbiorów danych.
Każde źródło ma `dataset_relationship`, `dataset_group` i
`dataset_scope_completeness`. Tabela najwyższego poziomu `dataset_groups`
zapisuje relację, członkostwo i kompletność zadeklarowanego zestawu członków.

Agregacja działa bezpiecznie w razie braku dowodów:

- `independent`: dokładnie jedno źródło w grupie, dodawane raz.
- `replica`: zgodne kopie liczą się raz. Przy rozbieżności zachowywany jest
  deterministyczny reprezentant, bez uśredniania, a wynik jest niekompletny.
- `shard`: członkowie są sumowani tylko wtedy, gdy
  `members_complete = true` i każdy zadeklarowany członek zakończył się
  powodzeniem. Niepełna grupa nie wnosi sum.
- `unknown`: wszystkie sumy tabel, wierszy i bajtów między źródłami są
  wyłączane.
- Źródło z niepełnym lub nieznanym `[dataset_scope]` oznacza dowód zbiorczy jako
  niepełny nawet przy znanej relacji.

Sumy dla poszczególnych źródeł są zawsze zachowane. Wyłączenie dotyczy tylko
agregatu między źródłami, dzięki czemu repliki nie są mnożone, a część shardów
nie jest przedstawiana jako cały zbiór.

## Manifest wsadowy

Utwórz manifest:

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
dataset_relationship = "independent"
tags = ["critical", "erp"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
dataset_relationship = "independent"
tags = ["billing"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
dataset_relationship = "independent"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
dataset_relationship = "independent"
tags = ["lake"]
```

Jeśli relacja zostanie pominięta, domyślną wartością jest `unknown`; przebieg
kończy się powodzeniem, lecz emituje `DBP1414W` i `DBP1417W` oraz wyłącza sumy zbiorcze. Jest
to bezpieczniejsze niż założenie, że dwa endpointy to dwa niezależne zbiory.

Zadeklaruj członków replikacji we wspólnej grupie:

```toml
[[source]]
id = "orders_primary"
kind = "postgresql"
connect_env = "ORDERS_PRIMARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true

[[source]]
id = "orders_secondary"
kind = "postgresql"
connect_env = "ORDERS_SECONDARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true
```

Dla systemów shardowanych wymień każdy znany shard we wspólnej grupie i ustaw
`dataset_group_complete = true` tylko wtedy, gdy manifest obejmuje kompletny
logiczny zbiór. Niepowodzenie członka czyni grupę niepełną w tym przebiegu.

Najpierw wykonaj przebieg próbny:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Uruchom zadanie wsadowe:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Przebieg wsadowy, który nie jest przebiegiem próbnym, wymaga `--yes`, ponieważ
może łączyć się z wieloma bazami danych lub dekodować próbki plików
strukturalnych. Każde źródło podrzędne otrzymuje własny plik audytu.

Przy `continue_on_error = true` pozostałe źródła są przetwarzane, a pakiet diagnostyczny wraz z `errors.txt` jest publikowany atomowo. Polecenie mimo to kończy się błędem: `DBP1115E`, gdy zawiodły wszystkie źródła, oraz `DBP1116E` przy błędzie częściowym. Pakiet częściowy służy do przeglądu i ponowienia; nie jest udanym pełnym zbiorem.

Zarówno przebieg próbny, jak i wykonanie sprawdzają cały manifest przed
uzyskaniem dostępu do źródła. Nieznane pola, zduplikowane identyfikatory,
identyfikatory kolidujące po bezpiecznej normalizacji nazwy pliku, pola
niezgodne z rodzajem źródła, niejednoznaczne źródła połączenia z bazą danych,
nieprawidłowe tryby zestawu danych oraz zerowe budżety próbkowania kompresji są
odrzucane. Każdy `source.id` musi być unikatowy, bez początkowych ani końcowych
spacji, i po normalizacji nie może przekraczać 120 bajtów ASCII.

## Tryby zestawów danych plików strukturalnych

Dla źródeł Parquet i Avro:

- `single_file` wymaga dokładnie jednego rozpoznanego pliku i zachowuje go jako jedną tabelę logiczną.
- `one_table_per_file` odwzorowuje każdy plik na oddzielną anonimowo oznaczoną tabelę w
  jednym podrzędnym pliku Blueprint.
- `merge_same_schema` scala wiele plików w jedną tabelę logiczną, gdy liczby
  kolumn są zgodne.
- `partitioned_dataset` zachowuje się jak `merge_same_schema`.

Sprawdzanie scalania jest celowo konserwatywne. Wymaga zgodności zanonimizowanego
układu kolumn, typów kanonicznych i natywnych, dopuszczalności wartości NULL,
zadeklarowanych szerokości, precyzji i skali, semantyki bez znaku i `BIT(n)`,
precyzji znaczników czasu, zestawu znaków i porównywania oraz semantyki źródła
strukturalnego. W planowaniu jeziora danych o dużej wadze grupuj zestawy danych
według znanego schematu nawet wtedy, gdy ta kontrola strukturalna zakończy się
powodzeniem.

## Operacje na pakietach

Wyświetl źródła:

```bash
dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Pierwsze wiersze pokazują `aggregation`, fizyczne `sources`,
`logical_datasets`, sumy zbiorcze i `limitations`. Wiersze grup pokazują
`relationship`, `members_complete` i identyfikatory źródeł. Wiersze źródeł
pokazują `dataset_relationship`, `dataset_group` i `dataset_scope`.
`aggregation=suppressed` oznacza konieczność sprawdzenia lub poprawienia
manifestu, a nie środowisko o zerowym rozmiarze.

Wyświetl jeden podzbiór źródeł oznaczony tagiem:

```bash
dbwarp-blueprint \
  --bundle-list customer-blueprint-bundle/bundle.toml \
  --select tag=erp
```

Wyodrębnij jedno źródło:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Wyodrębnij jedną tabelę z jednego źródła:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Obsługiwane klucze selektora to:

- `source=ID`
- `table=ID`
- `engine=postgresql|mysql|sqlserver|parquet|avro`
- `tag=NAME`

Selektory można przekazać jako jeden ciąg rozdzielony przecinkami albo jako
powtarzające się opcje `--select`. Sprzeczne wartości tego samego klucza są
odrzucane.

## Złożoność artefaktów w pakietach.

Złożoność artefaktów pozostaje cechą każdego poszczególnego Blueprintu. Nigdy nie jest ona sumowana, uśredniana ani przypisywana do poziomu zestawu: źródła mogą korzystać z różnych silników, wersji analizatorów, dialektów, profili gramatycznych, zakresów i populacji podlegających ocenie, więc agregacja nie miałaby uzasadnionego znaczenia. Porównaj każde źródło niezależnie.

## Granica prywatności i przeglądu

Pakiet nie rozluźnia modelu prywatności:

- źródła działających baz danych nadal emitują anonimowe identyfikatory tabel,
  kolumn i indeksów chronione tajnym kluczem;
- wartości Parquet są dekodowane tylko z `--measure-compression --yes`;
  przechwytywanie Avro zawsze przechodzi przez rekordy kontenera, aby wyznaczyć
  liczniki, długości i udziały wartości null, nawet bez pomiaru kompresji;
- zdekodowane próbki pozostają w pamięci;
- Metadane pakietu wykorzystują identyfikatory i tagi źródła, które zostały przez Ciebie wybrane;
- żadne polecenie pakietu nie wysyła telemetrii ani nie przesyła plików.

Można usunąć dowolny podrzędny element Blueprint lub wpis źródłowy przed udostępnieniem pakietu.

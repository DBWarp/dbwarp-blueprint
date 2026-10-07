<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../README.md). Zobacz [zasady tłumaczenia dokumentacji](../TRANSLATIONS.md).

[English](../../README.md) | [Deutsch](../de/README.md) | [Français](../fr/README.md) | [Español](../es/README.md) | [Polski](README.md) | [日本語](../ja/README.md) | [简体中文](../zh/README.md)

Wersja angielska jest wiążąca; tłumaczenia wspomagane maszynowo mają charakter
uzupełniający i mogą zawierać błędy.

## Czym jest to narzędzie

DBWarp Blueprint to kolektor Blueprint bazy danych zaprojektowany z myślą o zaufaniu. Uruchamiasz go we własnym środowisku względem PostgreSQL, MySQL lub SQL Server. Odczytuje metadane katalogowe, a tylko na żądanie pomiaru kompresji również ograniczoną próbkę wierszy. Następnie zapisuje zanonimizowany strukturalny Blueprint bazy danych: rozmiary tabel, liczby wierszy, rodziny typów oraz strukturę indeksów i kluczy obcych.

Identyfikatory są zastępowane anonimowymi etykietami przypisanymi za pomocą kluczy, a żadne wartości wierszy nie są zapisywane do Blueprint. Świeży, lokalny dla procesu klucz domyślnie uniemożliwia sprawdzanie słownika w trybie offline; `--anonymization-key-file` umożliwia zachowanie etykiet podczas zatwierdzonych porównań. Przed udostępnieniem jakichkolwiek wyników, przeczytaj [`SECURITY.md`](SECURITY.md): dokument ten dokładnie określa, co każdy tryb ujawnia oraz które opcje to rozszerzają.

Wynik jest plikiem tekstowym. Przed podjęciem decyzji o jego udostępnieniu możesz przeczytać każdy wiersz.

DBWarp Blueprint jest bezpłatnym oprogramowaniem typu open source i działa w całości w Twoim środowisku. Powstał po to, aby umożliwić przekazanie nam faktów o bazie danych bez przekazywania samej bazy danych.

## Dlaczego warto go uruchomić

Udostępnij nam wynik Blueprint, a będziemy mogli określić, o ile szybciej DBWarp przeniósłby Twoje dane i jaki miałoby to wpływ na harmonogram migracji, przygotowywania danych testowych CI/CD oraz analiz.

Najważniejsza jest odległość. Im dalej muszą zostać przesłane dane, tym większą poprawę może wykazać DBWarp.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zurych, Szwajcaria

---

`dbwarp-blueprint` jest działającym po stronie klienta kolektorem Blueprint dla DBWarp. Uruchamia się go we własnym środowisku klienta, aby utworzyć ograniczony, zanonimizowany i możliwy do przeglądu plik `blueprint.toml`, którego DBWarp może użyć do wymiarowania migracji, generowania syntetycznych zestawów danych i planowania wstępnego bez otrzymywania dostępu do bazy danych, zrzutów, nazw schematów ani danych wierszy.

Narzędzie łączy się z PostgreSQL, MySQL lub SQL Server, odczytuje metadane katalogu, opcjonalnie mierzy lokalną kompresję na podstawie ograniczonej próbki wierszy i zapisuje TOML w zwykłym tekście. Może również wyprowadzić Blueprint z lokalnych plików Parquet lub Avro w trybie offline, gdy dane wejściowe są już plikiem strukturalnym, a nie bazą danych na żywo. Możesz otworzyć dane wyjściowe, przejrzeć każdy wiersz i zdecydować, czy je udostępnić.

Opcjonalnie `--deck blueprint.pptx` zapisuje również podsumowanie PowerPoint tego samego zanonimizowanego Blueprint. Prezentację można zapisać podczas pracy z bazą danych na żywo albo później ze sprawdzonego pliku TOML za pomocą `--from-toml blueprint.toml --deck blueprint.pptx`. Mechanizm tworzenia prezentacji jest wbudowany w plik binarny Rust i nie nawiązuje połączeń sieciowych.

## Do czego służy

DBWarp potrzebuje wystarczającej ilości informacji strukturalnych, aby oszacować i zaplanować transfer:

- liczby tabel;
- przybliżonej liczby wierszy;
- rozmiarów tabel i indeksów;
- rodzin typów kolumn, dokładnych pojemności strukturalnych/prefiksów indeksów
  oraz domyślnie zaokrąglonych dla prywatności zaobserwowanych szerokości;
- struktury indeksów i kluczy obcych;
- ograniczonych, pozbawionych nazw liczników artefaktów innych niż tabele i zewnętrznych wymagań wdrożeniowych;
- opcjonalnych podsumowań kompresji tabel i kolumn z małej lokalnej próbki;
- opcjonalny pomiar czasu odpowiedzi (round-trip time) z narzędzia do zbierania danych do Twojej bazy danych.

Te informacje są wystarczające do oszacowania rozmiaru transferu i zaplanowania go. Nazwy źródeł i wartości w wierszach są pomijane, ale charakterystyczna struktura i statystyki nadal mogą identyfikować obciążenie; anonimizacja to redukcja ryzyka, a nie gwarancja nieodwracalności.

## Czego nie robi

Narzędzie `dbwarp-blueprint`:

- nie wysyła telemetrii;
- nie wywołuje serwerów DBWarp;
- nie przesyła pliku Blueprint;
- nie odczytuje `~/.pgpass`, `~/.my.cnf`, poświadczeń chmurowych ani kluczy SSH;
- nie odczytuje domyślnych zmiennych środowiskowych haseł, takich jak `PGPASSWORD` lub `MYSQL_PWD`;
- nie używa niejawnych systemowych katalogów tymczasowych, pamięci podręcznej ani
  konfiguracji; zapisuje jawnie wybrane pliki wyjściowe, a tryb wsadowy używa także
  sąsiedniego katalogu przygotowania lub odzyskiwania obok `--out-dir` do atomowej publikacji;
- nie umieszcza w danych wyjściowych rzeczywistych nazw tabel, kolumn, indeksów ani schematów, nazw obiektów innych niż tabele, definicji SQL, zewnętrznych punktów końcowych, poświadczeń, kluczy, certyfikatów, plików binarnych ani wartości wierszy.

Uruchomienie Blueprint na żywo otwiera sesję bazy danych z podanym punktem
końcowym. DNS może korzystać ze skonfigurowanego resolvera, a zintegrowane
uwierzytelnianie Kerberos/SSPI może kontaktować się z infrastrukturą tożsamości.
Tryb wsadowy powtarza tę granicę dla każdego źródła bazodanowego. Lokalne
operacje TOML, Parquet, Avro i operacje na pakietach nie inicjują połączeń
sieciowych aplikacji.

## Pobieranie lub budowanie

| Ścieżka | Najlepsze zastosowanie | Odsyłacz |
|---|---|---|
| Pobierz plik binarny. | szybka wersja testowa, odizolowany serwer testowy. | [`binaries/README.md`](BINARIES.md) |
| Budowanie z małego klonu źródeł | przegląd bezpieczeństwa, zasady produkcyjne, kontrola odtwarzalności | [`BUILD.md`](BUILD.md) |
| Budowanie z pakietu źródeł z zależnościami | rygorystyczny audyt zależności offline | GitHub Releases |
| Przegląd i uruchomienie zapasowej ścieżki SQL | zasady DBA odrzucają plik binarny strony trzeciej | [`sql/blueprint.pg.sql`](../../sql/blueprint.pg.sql), [`sql/blueprint.mysql.sql`](../../sql/blueprint.mysql.sql), [`sql/blueprint.sqlserver.sql`](../../sql/blueprint.sqlserver.sql) i [`blueprint_format.py`](../../blueprint_format.py) |

### Granica zapasowej ścieżki SQL

Zapasowa ścieżka SQL jest możliwym do przeglądu minimum katalogowym, a nie funkcjonalnym odpowiednikiem kolektora Rust. Każdy skrypt SQL zapisuje pośredni dokument JSON z rzeczywistymi nazwami schematów, tabel, kolumn i indeksów; w MySQL `COLUMN_TYPE` może również zawierać zadeklarowane elementy enum/set. Traktuj ten JSON jako wrażliwy materiał schematu, pozostaw go w środowisku źródłowym, znormalizuj lokalnie za pomocą `blueprint_format.py` i udostępniaj wyłącznie sprawdzony wynik TOML.

Ścieżka zapasowa nie ma selektora `--schema`: PostgreSQL obejmuje zwykłe tabele we wszystkich schematach niesystemowych połączonej bazy, a MySQL i SQL Server zwykłe lokalne tabele użytkownika w wybranej bazie. Nie używaj jej, gdy zatwierdzono tylko podzbiór. Jej TOML zawiera podzbiór zwykłych tabel ze strukturą tabel, kolumn, indeksów i FK oraz przybliżonymi lokalnymi rozmiarami, ale nie inwentaryzuje wszystkich rodzajów tabel v7. Oznacza wszystkie rodziny struktury jako niekompletne oraz wyklucza tabele MySQL FEDERATED i zewnętrzne tabele SQL Server, zamiast błędnie oznaczać ich zdalne dane jako lokalne. Nie zawiera także próbkowania wierszy, dowodów RTT, inwentarza artefaktów innych niż tabele ani sond topologii na żywo; topologia i kompletność zbioru danych są jawnie `unknown`.

Ścieżką stawiającą zaufanie na pierwszym miejscu jest budowanie ze źródeł. Zwykłe repozytorium pozostaje małe i używa `Cargo.lock` do przypięcia wersji zależności. Na potrzeby bardziej rygorystycznych audytów offline każde wydanie publikuje również pakiet źródeł ze wszystkimi plikami źródłowymi zależności. Dla wygody dostarczane są pliki binarne wydania z sumami kontrolnymi SHA256.

## Szybki start

Przed każdym uruchomieniem na żywej bazie poproś DBA o utworzenie dedykowanego
konta kolektora z minimalnymi uprawnieniami za pomocą skryptu odpowiedniego dla
silnika, wersji i poziomu z [`sql/grants/`](../../sql/grants/) oraz o
zatwierdzenie dokładnego zakresu schematów. Nigdy nie zaczynaj od konta
właściciela aplikacji ani administratora. Każdy zatwierdzony schemat przekaż
przez `--schema`, a po przechwyceniu usuń dedykowane konto odpowiednim skryptem
z [`sql/revoke/`](../../sql/revoke/). Pełna procedura pierwszego uruchomienia
znajduje się w [`docs/QUICKSTART.md`](QUICKSTART.md).

W razie potrzeby wybierz język prezentacji. Domyślny jest angielski; kompletne
katalogi są wbudowane dla języka niemieckiego, francuskiego, hiszpańskiego,
polskiego, japońskiego i chińskiego uproszczonego:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --schema app --dry-run
```

Tłumaczone są wyłącznie pomoc, monity, diagnostyka, postęp i etykiety
prezentacji PowerPoint przeznaczone dla człowieka. Nazwy poleceń i opcji,
akceptowane wartości, schematy URI, nazwy zmiennych środowiskowych, selektory,
kody DBP, klucze audytu i generowany TOML pozostają kanonicznymi tokenami
angielskimi. Dzięki temu automatyzacja i procedury pomocy są identyczne w każdym
języku. Zobacz [`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Przed połączeniem z bazą danych, zapoznaj się z przykładami dostępnymi pod adresem [`samples/`](../../samples/). Są to standardowe pliki TOML w formacie Blueprint i nie wymagają żadnej konfiguracji do przeglądania. Po pobraniu pliku binarnego, można uruchomić go w trybie offline, aby wygenerować prezentację bez dostępu do bazy danych ani sieci:

```bash
./dbwarp-blueprint --from-toml samples/sqlserver-v6-analyzed.toml --deck sample.pptx
```

Zacznij od małych przykładów Blueprint opisanych w [`samples/README.md`](../../samples/README.md): katalog PostgreSQL, próbki MySQL i próbki SQL Server z analizowanymi artefaktami. Są to przykłady ręcznie przygotowane, a nie rzeczywiste dane. Większe przykłady schematu v1 pokazują starszy format, który jest nadal czytelny. Użyj [`FORMAT.md`](FORMAT.md), aby zapoznać się z pełnym formatem wyjściowym przed zatwierdzeniem zebrania danych; żaden przykład nie obejmuje wszystkich opcjonalnych pól.
Najpierw wykonaj przebieg próbny. Wypisuje plan bez nawiązywania połączenia:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --dry-run
```

Zalecane uruchomienie w stylu produkcyjnym z TLS, dziennikiem audytu i pomiarem kompresji:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Dzięki `--measure-compression --yes` wynik zawiera współczynniki kompresji zstd na poziomie tabeli oraz prognozy kompresji dla poszczególnych kolumn. Bloki dla poszczególnych kolumn są obliczane na podstawie tego samego, ograniczonego zbioru danych, co współczynnik na poziomie tabeli; służą one do doprecyzowania oszacowania transferu i nie zapisują próbek danych na dysku. Materiałnie dominująca, binarna próbka z rozpoznawalnymi, standardowymi sygnaturami skompresowanych kontenerów otrzymuje tylko ogólną etykietę `style = "precompressed"`; nie jest serializowany żaden typ pliku, sygnatura ani wartość próbki. Schematy w wersji 3 i nowszych również generują ograniczone, pozbawione nazw agregaty dla poszczególnych kolumn cardinality/skew oraz wnioskowane podsumowania index-prefix/relationship. Tymczasowe hasze dla poszczególnych wartości są ograniczone w pamięci i usuwane; próbki danych i hasze dla poszczególnych wartości nigdy nie pojawiają się w pliku Blueprint TOML, podczas gdy udokumentowane agregaty tak.

Od schematu v4 Blueprinty inwentaryzują również obiekty inne niż tabele. Domyślne
`--artifact-detail summary` zapisuje ograniczone liczniki według klas obiektów i
zewnętrznych wymagań bez odczytywania definicji. `graph` dodaje anonimową
topologię zależności, a `analyzed` ograniczone przedziały cech języka i
złożoności. Oba wymagają `--yes`, ponieważ nawet anonimowy graf może
identyfikować aplikację:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


Obecność artefaktu jest dowodem planistycznym, a nie deklaracją, że DBWarp może
go automatycznie odtworzyć lub przetłumaczyć. Zobacz
[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

### Wierność długości MySQL

Domyślna polityka `balanced` zachowuje dokładnie zadeklarowane pojemności
znakowe/bajtowe i długości prefiksów indeksów. Próbkowane średnie/p95 długości
wartości korzystają z przedziałów błędu względnego (maksymalny błąd około 3,2%,
przy czym wartości do 32 bajtów zachowuje się dokładnie). Dzięki temu klucz
`VARCHAR(3000)`, którego wartości zwykle mają 9 znaków, pozostaje blisko 9
znaków, dzięki czemu rozmiar odzwierciedla rzeczywistą szerokość wartości, a
jednocześnie zachowane są prawidłowe limity DDL/indeksów źródła:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

Używaj dokładnych statystyk próbek tylko wtedy, gdy zasady pozwalają na dodatkową precyzję:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Użyj `--length-fidelity strict` do zastosowania ogólnego podziału danych pod względem prywatności dla zadeklarowanych, zaobserwowanych i prefiksów. Tryb ścisły zmniejsza dokładność uzyskiwanego wyniku. Pisownia `--preserve-exact-lengths --yes` pozostaje aliasem dla `--length-fidelity exact --yes`.

Nowe plany (Blueprints) zawierają oddzielne pola `declared_length_fidelity`, `index_length_fidelity` i `observed_length_fidelity`. Pole `length_metadata` również jest zapisywane, aby narzędzia odczytujące starszy format mogły nadal działać. Dokładne wartości katalogu dla pojemności znaków w PostgreSQL są dostępne; limity bajtów zależne od kodowania oraz długości prefiksów indeksów pozostają niedostępne.

Dla uzyskania najbardziej dokładnej oszacowania, uruchom z opcją `--measure-compression`: rejestruje ona obserwowane średnie i wartości p95 długości, dzięki czemu kolumna zadeklarowana jako znacznie szersza niż jej rzeczywiste wartości nie zostanie przeszacowana. Domyślny limit czasu próbkowania wynosi 300 sekund; zwiększ wartość `--max-wall-secs` dla bardzo dużych schematów.

Następnie przejrzyj pliki:

```bash
less blueprint.toml
less audit.txt
```

Przed wysłaniem jakiegokolwiek elementu, należy wykonać [kroki przeglądania i udostępniania](QUICKSTART.md#review-and-share). Udostępniaj tylko zatwierdzone treści Blueprint, a w razie potrzeby, po oddzielnym przeglądzie i zatwierdzeniu, prezentację; domyślnie, zachowuj lokalnie dokumentację operacyjną.

## Tryb pliku strukturalnego

Jeżeli źródłem jest już lokalny plik strukturalny, wygeneruj TOML Blueprint bez poświadczeń bazy danych:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Tryb Parquet odczytuje stopkę i metadane grup wierszy. Kontenery obiektów Avro nie mają równoważnej liczby wierszy w stopce, dlatego tryb Avro przechodzi przez kontener, aby policzyć rekordy, i używa schematu zapisu (writer schema) do określenia struktury kolumn. Żaden z trybów nie łączy się z bazą danych ani nie odczytuje flag poświadczeń.

Jeżeli zasady zezwalają na zdekodowane próbkowanie, tryb plikowy może również
mierzyć ograniczoną lokalną podatność na kompresję na potrzeby planowania
transferu:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Te same flagi działają z `--from-avro`. Próbkowane wartości są kodowane w
pamięci jako `blueprint-compression-probe-v2`; Blueprint przechowuje zagregowane
pomiary kompresji, gęstości NULL, liczności/częstotliwości, długości i stylu,
nigdy próbkowane wartości.

## Tryb wsadowy i pakietowy

Dla wielu baz danych, wielu tables/datasets lub w celu przeglądu całej infrastruktury, użyj zbiorczego manifestu i utwórz katalog zawierający wyniki:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Katalog roboczy zawiera `bundle.toml`, podrzędne pliki Blueprint dla
poszczególnych źródeł oraz objęte kontrolą dostępu dzienniki audytu. Domyślnie
nie przekazuj całego katalogu roboczego. Możesz wyświetlić lub wyodrębnić jego
zawartość albo utworzyć oddzielnie sprawdzony, spakowany pakiet Blueprint:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

Składnia manifestu, tryby zestawów plików strukturalnych i zasady selektorów są
opisane w [`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md).

## Typowe polecenia dla baz danych

PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --schema payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --schema dbo \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Przykłady Kerberos, SSPI i Entra ID znajdują się w [`AUTH.md`](AUTH.md). Informacje o wewnętrznych CA, mTLS i weryfikacji nazwy hosta znajdują się w [`TLS.md`](TLS.md).

## Tryb wyłącznie katalogowy

Jeśli zasady dopuszczają wyłącznie katalogi tabel, kolumn, indeksów i FK, pomiń `--measure-compression` i jawnie wyłącz domyślne podsumowanie obiektów innych niż tabele:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

Ten tryb, działający tylko na katalogach, odczytuje metadane tabel, statystyki oraz sondę topologii, która zlicza obiekty, ale nie odczytuje wartości wierszy ani informacji o obiektach innych niż tabele. DBWarp nadal może oszacować rozmiar na podstawie rozmiaru tabeli, liczby wierszy, typów danych oraz index/FK struktury, ale oszacowania kompresji są słabsze, ponieważ text/binary entropia musi być wnioskowana. Bez `--artifact-detail none`, domyślne podsumowanie odczytuje również katalogi obiektów innych niż tabele, ale nie ich definicje.

## Podgląd danych wyjściowych

```toml
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 7
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 1
row_count = 12500000
table_bytes = 4194304000
index_bytes = 1048576000

[database_topology]
contract = "dbwarp-blueprint-topology/v2"
deployment = "unknown"
local_role = "unknown"
visibility = "unknown"
member_count = 0
member_count_scope = "unknown"
identifiers_redacted = true

[dataset_scope]
contract = "dbwarp-blueprint-dataset-scope/v1"
layout = "unknown"
table_inventory_completeness = "unknown"
row_count_completeness = "unknown"
size_completeness = "unknown"
row_count_method = "postgres-planner-estimate"
size_method = "postgres-local-relation-size"
limitations = ["topology-unobserved", "topology-visibility-unknown"]

[structure_scope]
contract = "dbwarp-blueprint-structure-scope/v1"
visibility = "unknown"
table_inventory_completeness = "unknown"
column_inventory_completeness = "unknown"
index_inventory_completeness = "unknown"
relationship_inventory_completeness = "unknown"
limitations = ["metadata-visibility-unknown"]

[source_environment]
contract = "dbwarp-blueprint-source-environment/v1"
evidence_origin = "database-endpoint"
hosting_model = "unknown"
infrastructure_location = "unknown"
capacity_scope = "connected-instance"
capacity_visibility = "partial"
cpu_capacity_band = "unknown"
cpu_capacity_basis = "unknown"
memory_capacity_band = "under-2-gib"
memory_capacity_basis = "database-buffer-cache"
collector_machine_excluded = true
catalogs_read = ["pg-capacity-settings"]

[statistics_evidence]
contract = "dbwarp-blueprint-statistics-evidence/v1"
visibility = "unknown"
table_count = 1
counts_by_statistics_state = { unknown = 1 }
counts_by_row_count_quality = { unknown = 1 }
counts_by_size_quality = { unknown = 1 }
limitations = ["statistics-provenance-unclassified"]

[artifact_inventory]
contract = "dbwarp-blueprint-artifacts/v2"
detail = "none"
scope = "all-visible-schemas"
visibility = "unknown"
inventory_complete = false
dependencies_complete = false
requirements_complete = false
analysis_complete = false
families_not_inventoried = ["non_table_objects"]

[tables.table-001]
rows = 12500000
table_bytes = 4194304000
index_bytes = 1048576000
schema = "schema-A"
has_clustered_index = false
object_kind = "ordinary-table"
storage_organization = "unknown"
partitioning = "none"
segment_state = "unknown"

[tables.table-001.statistics]
row_count_method = "postgres-planner-estimate"
row_count_quality = "unknown"
statistics_state = "unknown"
refresh_age_band = "unknown"
modification_ratio_band = "unknown"
sample_fraction_band = "unknown"
statistics_scope = "unknown"
size_method = "postgres-local-relation-size"
size_quality = "unknown"
size_scope = "unknown"
size_accounting = "unknown"
size_visibility = "unknown"

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false
numeric_model = "integer"
numeric_precision_radix = "decimal"

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

Pełny kontrakt pliku opisano w [`FORMAT.md`](FORMAT.md). Dziennik audytu opisano w [`AUDIT.md`](AUDIT.md).

## Wizualna prezentacja podsumowująca

Wygeneruj prezentację podczas uruchomienia na żywo:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

Albo zbuduj ją później ze sprawdzonego pliku Blueprint, bez połączenia z bazą danych:

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

Prezentacja dostosowuje się do wielkości schematu: szczegóły poszczególnych tabel dla małych schematów, slajdy charakteryzujące dla dużych schematów, podsumowanie kompresji, gdy dostępne są dane poziomu 2, oraz slajd modelu zaufania. Zobacz [`DECK.md`](DECK.md).

## Dokumentacja

Zacznij tutaj:

- [`docs/QUICKSTART.md`](QUICKSTART.md): pierwsze bezpieczne uruchomienie i co należy udostępnić.
- [`docs/COOKBOOK.md`](COOKBOOK.md): praktyczne przepisy dla PostgreSQL, MySQL, SQL Server, TLS, prezentacji i przepływów bez próbkowania.
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md): informacje potrzebne DBA lub recenzentowi bezpieczeństwa przed uruchomieniem narzędzia.
- [`sql/grants/README.md`](../../sql/grants/README.md): uwzględniające wersję skrypty minimalnych uprawnień i usuwanie konta po przechwyceniu.
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md): typowe awarie i sposoby ich rozwiązania.
- [`docs/MESSAGES.md`](MESSAGES.md): stabilne kody komunikatów operatorskich `DBPnnnnS`.
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md): sposób działania próbkowania kompresji poziomu 2.
- [`docs/INDEX.md`](INDEX.md): kompletna mapa dokumentacji.

Punkty wyjścia do przeglądu bezpieczeństwa:

- [`SECURITY.md`](SECURITY.md): model bezpieczeństwa i obsługa poświadczeń.
- [`AUDIT.md`](AUDIT.md): co jest odczytywane, zapisywane, odpytywane i rejestrowane.
- [`FORMAT.md`](FORMAT.md): pola danych wyjściowych i zasady zaokrąglania.
- [`TLS.md`](TLS.md): zachowanie TLS i mTLS.
- [`AUTH.md`](AUTH.md): obsługiwane tryby uwierzytelniania.
- [`BUILD.md`](BUILD.md): budowanie ze źródeł i weryfikacja wydania.
- [`DECK.md`](DECK.md): opcjonalna prezentacja podsumowująca PowerPoint.

## Licencja

Apache-2.0 OR MIT.

# Przepisy

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../COOKBOOK.md).

[English](../COOKBOOK.md) | [Deutsch](../de/COOKBOOK.md) | [Français](../fr/COOKBOOK.md) | [Español](../es/COOKBOOK.md) | [Polski](COOKBOOK.md) | [日本語](../ja/COOKBOOK.md) | [简体中文](../zh/COOKBOOK.md)

Przepisy ukierunkowane na zadania dla typowych przepływów pracy `dbwarp-blueprint`.

## Przepis: zlokalizowana sesja operatorska

Wybierz jeden z kompletnych wbudowanych katalogów językowych, zachowując
kanoniczne polecenia, wartości, identyfikatory i schematy danych wyjściowych:

```bash
./dbwarp-blueprint --lang de --help
./dbwarp-blueprint --lang ja \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

Dla uruchomień bez nadzoru ustaw `DBWARP_BLUEPRINT_LANG=fr` albo standardowe
ustawienia regionalne procesu. Jawne `--lang` zawsze ma pierwszeństwo. Kody DBP
i niskopoziomowe szczegóły dostawcy pozostają kanoniczne, dzięki czemu
zlokalizowany błąd można wyszukać i przekazać pomocy technicznej.

## Przepis: PostgreSQL z wewnętrznym CA

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out pg-appdb.blueprint.toml \
  --audit-log pg-appdb.audit.txt
```

Używaj tego wariantu do zwykłego przeglądu produkcyjnego PostgreSQL. Jeżeli weryfikacja nazwy hosta nie powiedzie się, popraw certyfikat serwera albo użyj właściwej nazwy DNS; nie używaj `--tls-skip-verify` poza testami pętli zwrotnej.

## Przepis: MySQL z plikiem nazwy użytkownika

Przydatne, gdy nazwa użytkownika zawiera znaki trudne do zakodowania w URI.

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

Aby uzyskać syntetyczną rekonstrukcję reprezentatywną pod względem wydajności,
użyj domyślnej polityki `balanced`: dokładnych metadanych deklaracji/indeksów
MySQL i ściśle zaokrąglonych szerokości próbek:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

Potwierdź `declared_length_fidelity = "exact"`,
`index_length_fidelity = "exact"` oraz
`observed_length_fidelity = "relative-rounded-v2"`. Użyj
`--length-fidelity exact --yes` dopiero po zatwierdzeniu przez klienta
udostępniania dokładnych statystyk długości próbek. Nazwy i wartości pozostają
wykluczone.

W środowiskach z tysiącami tabel zwiększ w razie potrzeby `--max-wall-secs`
powyżej domyślnej wartości 300 sekund. Znaczniki wierności poświadczają politykę,
a estymator w kolejnym etapie niezależnie wymaga zaobserwowanych długości
średnich/p95 dla każdej niepustej indeksowanej kolumny o zmiennej szerokości,
zanim oznaczy zestaw danych jako gotowy do benchmarku.

## Przepis: uwierzytelnianie SQL w SQL Server

```bash
./dbwarp-blueprint \
  --connect sqlserver://sql-blueprint@sql-primary.internal,1433/appdb \
  --password-file /etc/dbwarp/sql-blueprint.pass \
  --auth-mode sql-auth \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-appdb.blueprint.toml \
  --audit-log mssql-appdb.audit.txt
```

Tryby TLS SQL Server weryfikujące certyfikat używają magazynu zaufania systemu
operacyjnego, gdy pominięto `--tls-ca`. Dostarczony plik `.pem` lub `.crt` musi
zawierać dokładnie jeden certyfikat CA i zastępuje te korzenie. Zarówno
`verify-ca`, jak i `verify-full` sprawdzają nazwę hosta połączenia.

## Przepis: token Entra ID dla SQL Server

Wygeneruj token poza narzędziem, a następnie przekaż go przez plik:

```bash
install -d -m 700 "$HOME/.cache/dbwarp-blueprint"
TOKEN_FILE="$HOME/.cache/dbwarp-blueprint/sql-token"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"
chmod 600 "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

## Przepis: przegląd bezpieczeństwa tylko katalogu

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

Jest to tryb przeglądu o najmniejszym oporze wdrożeniowym. Pomija próbkowanie wierszy, ale skutkuje mniej dokładnymi dalszymi oszacowaniami kompresji i ruchu wychodzącego.

## Oceń złożoność migracji obiektów innych niż tabele

Zacznij od domyślnego podsumowania, aby zebrać liczniki i zewnętrzne wymagania bez odczytywania definicji:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail summary \
  --out appdb-summary.blueprint.toml \
  --audit-log appdb-summary.audit.txt \
  --yes
```


Po zatwierdzeniu bezpieczeństwa zbierz anonimowe zależności i ograniczone dowody złożoności języka:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail analyzed \
  --out appdb-analyzed.blueprint.toml \
  --audit-log appdb-analyzed.audit.txt \
  --yes
```


Sprawdź `visibility`, wszystkie trzy flagi kompletności, `catalogs_unreadable`, `families_not_inventoried` oraz `counts_by_external_class`. Traktuj każdą klasę zewnętrzną jako osobne zadanie migracyjne. Zinwentaryzowany obiekt nie dowodzi, że DBWarp może go odtworzyć lub przetłumaczyć; porównaj go z macierzą możliwości migracji. Zobacz [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Przepis: wyłączenie pomiaru RTT

Domyślnie narzędzie wykonuje pięć prób `SELECT 1` po zestawieniu połączenia i generuje blok `[network]`. Jeżeli DBA zabrania zapytań niekatalogowych, wyłącz tę funkcję:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --no-rtt-probe \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Pomiar RTT nigdy nie odczytuje danych wierszy; każde zapytanie zwraca stałą liczbę całkowitą `1`.

## Przepis: ograniczenie czasowe próbkowania kompresji

Dla dużych systemów produkcyjnych zachowaj ostrożność podczas pierwszego uruchomienia:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal/appdb \
  --password-file /etc/dbwarp/mysql.pass \
  --measure-compression --yes \
  --sample-rows 500 \
  --max-wall-secs 120 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Jeżeli dane wyjściowe oznaczają wiele próbek jako obciążone lub brakujące, uruchom narzędzie ponownie na replice do odczytu z większym budżetem czasu.

## Przepis: jeden klient, wiele baz danych

Użyj manifestu wsadowego, gdy klient chce otrzymać jeden sprawdzony pakiet dla kilku baz danych.

`customer.batch.toml`:

```toml
[defaults]
measure_compression = true
sample_rows = 1000
max_wall_secs = 300
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["erp", "critical"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
tags = ["billing"]

[[source]]
id = "warehouse_sql"
kind = "sqlserver"
connect_env = "WAREHOUSE_SQL_URI"
password_file = "/etc/dbwarp/warehouse.pass"
auth_mode = "sql-auth"
tags = ["warehouse"]
```

Przebieg próbny:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Uruchomienie:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Powstają `bundle.toml`, po jednym Blueprint podrzędnym dla każdego źródła oraz po jednym audycie dla każdego źródła.
Blueprinty podrzędne można nadal przeglądać niezależnie.

## Przepis: jeden klient, mieszane bazy danych i pliki jeziora danych

Użyj źródeł plików strukturalnych w tym samym zadaniu wsadowym, gdy klient ma wyciągi Parquet lub Avro obok baz danych na żywo.

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["database"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/mnt/customer/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/mnt/customer/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` obecnie scala pliki podobnie jak `merge_same_schema`, ale zachowuje intencję klienta widoczną w pakiecie. Niepowiązane schematy przechowuj w oddzielnych źródłach.

## Przepis: wyodrębnienie tylko jednego źródła lub tabeli z pakietu

Po uruchomieniu wsadowym wyświetl źródła:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Wyodrębnij jedno źródło:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Wyodrębnij jedną tabelę z jednego źródła:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Użyj tej funkcji, gdy klient zatwierdza do benchmarku tylko część środowiska albo gdy chcesz wygenerować niewielki, ukierunkowany zestaw danych z dużego pakietu.

## Przepis: spakowanie oddzielnie sprawdzonego pakietu do przekazania

Roboczy katalog pakietu zawiera Blueprinty podrzędne i audyty objęte kontrolą dostępu. Nie przekazuj go w całości. Po sprawdzeniu wartości manifestu i Blueprintów podrzędnych utwórz plik do przekazania:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

Spakowany plik zachowuje identyfikatory źródeł, tagi, identyfikatory grup zestawów danych i metadane ścieżek audytu podane przez operatora. Użyj anonimowych wartości, sprawdź spakowany TOML i przekaż go wyłącznie zatwierdzonym kanałem.

## Przepis: wsadowy pakiet do przekazania

Utwórz katalog o następującej strukturze:

```text
customer-blueprint-handoff/
  customer-blueprint-bundle.packed.toml
  customer.batch.toml.redacted
  reviewer-notes.md       # optional
```

Utwórz ten oddzielny katalog ze sprawdzonych kopii. Zachowaj robocze `bundle.toml`, `blueprints/`, `audits/` oraz wszelkie pliki `errors.txt` lokalnie i pod kontrolą dostępu. `customer.batch.toml.redacted` powinien zawierać tylko zatwierdzone identyfikatory źródeł, rodzaje, tagi i tryby zestawów danych. Nie dołączaj sekretów, prywatnych nazw hostów, plików haseł, plików tokenów, kluczy prywatnych, dzienników bazy danych ani zdekodowanych próbek wierszy.

## Przepis: prezentacja offline ze sprawdzonego TOML

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

Ten tryb odczytuje wyłącznie plik TOML i zapisuje prezentację. Odrzuca flagi bazy danych na żywo, zamiast po cichu je ignorować.

## Przepis: odtwarzalność bajt w bajt

Ustal znacznik czasu:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Użyj tej funkcji do analizy kryminalistycznej, porównań migawek lub deterministycznego generowania prezentacji.

## Przepis: pakiet do przekazania do DBWarp

Utwórz katalog o następującej strukturze:

```text
customer-blueprint-handoff/
  blueprint.toml
  blueprint.pptx              # optional
  command-used.redacted.txt
  reviewer-notes.md           # optional
```

`command-used.redacted.txt` może rejestrować zatwierdzone flagi i budżety próbkowania, ale należy z niego usunąć dane uwierzytelniające, tokeny, prywatne nazwy hostów i ścieżki lokalne. Zachowaj `audit.txt` lokalnie jako dowód operacyjny objęty kontrolą dostępu. Dołącz go tylko w przypadku wskazanej potrzeby pomocy technicznej i przekaż zatwierdzonym bezpiecznym kanałem. Nie dołączaj plików haseł, plików tokenów, kluczy prywatnych ani dzienników bazy danych.

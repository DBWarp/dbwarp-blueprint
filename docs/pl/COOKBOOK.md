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
  --artifact-detail none \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

Dla uruchomień bez nadzoru ustaw `DBWARP_BLUEPRINT_LANG=fr` albo standardowe
ustawienia regionalne procesu. Jawne `--lang` zawsze ma pierwszeństwo. Kody DBP
i niskopoziomowe szczegóły sterownika pozostają kanoniczne, dzięki czemu
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

Powyższy przepis już wykorzystuje domyślną, zrównoważoną strategię: dokładne metadane MySQL declaration/index oraz ściśle zaokrąglone, pobrane zakresy wartości.

Potwierdź `declared_length_fidelity = "exact"`, `index_length_fidelity = "exact"` i `observed_length_fidelity = "relative-rounded-v2"`. Użyj `--length-fidelity exact --yes` tylko po uzyskaniu zgody organizacji na udostępnianie dokładnych statystyk dotyczących próbkowanych długości. Nazwy i wartości pozostają wyłączone.

W bazach danych zawierających tysiące tabel, w razie potrzeby, zwiększ wartość `--max-wall-secs` powyżej domyślnej wartości 300 sekund. Markery jakości opisują politykę; nie wskazują, że próbkowanie objęło wszystkie tabele.

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
install -m 600 /dev/null "$TOKEN_FILE"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

Azure SQL przedstawia certyfikat publicznego urzędu CA, dlatego ten przepis nie
ustawia `--tls-ca` i używa magazynu zaufania systemu operacyjnego. Podany plik
`--tls-ca` zastępuje ten magazyn jednym certyfikatem; zobacz [TLS](TLS.md).

## Przepis: przegląd bezpieczeństwa tylko katalogu

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

To jest tryb przeglądania o najmniejszym obciążeniu. Unika pobierania próbek wierszy, ale generuje mniej dokładne szacunki dotyczące kompresji i transferu danych.

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


Przejrzyj `visibility`, wszystkie trzy flagi kompletności, `catalogs_unreadable`, `families_not_inventoried` i `counts_by_external_class`. Traktuj każdą zewnętrzną klasę jako oddzielne zadanie migracji. Nie traktuj zindeksowanego obiektu jako dowodu na to, że DBWarp może go odtworzyć lub przetłumaczyć; zapytaj DBWarp, jakie typy obiektów są obsługiwane dla Twojej migracji. Zobacz [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

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

## Przepis: Kilka baz danych w jednym pakiecie.

Użyj manifestu wsadowego, gdy chcesz mieć jeden, sprawdzany pakiet dla kilku baz danych.

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

## Przepis: Połączenie różnych baz danych i plików z Data Lake.

Używaj źródeł w postaci plików strukturalnych w tej samej partii, gdy masz ekstrakcje w formacie Parquet lub Avro obok baz danych produkcyjnych.

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
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` łączy pliki, takie jak `merge_same_schema`, i rejestruje zadeklarowany tryb w pakiecie. Niezwiązane schematy należy przechowywać w oddzielnych źródłach.

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

Używaj tego, gdy tylko część pakietu jest zatwierdzona do udostępnienia.

## Instrukcja: Przygotuj i udostępnij sprawdzony zestaw wyników.

Katalog roboczy zawiera podrzędne projekty (Blueprints) oraz audyty z ograniczonym dostępem. Nie należy go przenosić w całości. Po przejrzeniu wartości w pliku manifest oraz podrzędnych projektach, utwórz pojedynczy plik do udostępnienia:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

Spakowany plik zachowuje identyfikatory źródeł, tagi, identyfikatory grup zestawów danych i metadane ścieżek audytu podane przez operatora. Użyj anonimowych wartości, sprawdź spakowany TOML i przekaż go wyłącznie zatwierdzonym kanałem.

## Przepis: Pakiet zbiorczy do udostępnienia.

Postępuj zgodnie z [instrukcjami dotyczącymi przeglądu i udostępniania](QUICKSTART.md#review-and-share). Przechowuj lokalnie aktualny manifest, raporty audytu i zapisy poleceń; utwórz ten oddzielny katalog tylko na podstawie sprawdzonych, spakowanych plików Blueprint.

```text
blueprint-share/
  customer-blueprint-bundle.packed.toml
```

## Przepis: prezentacja offline ze sprawdzonego TOML

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

Ten tryb odczytuje wyłącznie plik TOML i zapisuje prezentację. Odrzuca flagi bazy danych na żywo, zamiast po cichu je ignorować.

## Przepis: odtwarzalność bajt w bajt

Ustal znacznik czasu i ponownie użyj tego samego chronionego klucza
anonimizacji, który przechowujesz:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --anonymization-key-file /etc/dbwarp/anonymization.key \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Plik klucza musi zawierać dokładnie 32 bajty lub 64 znaki szesnastkowe, nie może mieć wartości group/world-readable w systemie Unix i nigdy nie może być udostępniany. Bez tej opcji, nowy klucz generowany przez system operacyjny celowo zmienia kolejność anonimowych etykiet przy każdym uruchomieniu. Ustawienie tylko `--generated-at` jest niewystarczające. Użyj pełnej procedury do tworzenia zatwierdzonych kopii zapasowych do celów kryminalistycznych; zestaw danych wygenerowany dwukrotnie z dokładnie tego samego, zweryfikowanego Blueprintu pozostanie identyczny pod względem bajtów, pod warunkiem, że jego znacznik czasu i język pozostaną niezmienione.

## Przepis: Pakiet do udostępnienia z DBWarp.

Postępuj zgodnie z [instrukcjami dotyczącymi przeglądu i udostępniania](QUICKSTART.md#review-and-share). Domyślny pakiet zawiera tylko zatwierdzony Blueprint:

```text
blueprint-share/
  blueprint.toml
```

Dodawaj `blueprint.pptx` tylko po wcześniejszym, oddzielnym sprawdzeniu i zatwierdzeniu. Przechowuj audyty, zapisy poleceń oraz materiały credential/key poza współdzielonym katalogiem; przesyłaj audyty tylko w przypadku konkretnych potrzeb wsparcia, za pośrednictwem zatwierdzonego, bezpiecznego kanału.

# Uwierzytelnianie

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../AUTH.md).

[English](../../AUTH.md) | [Deutsch](../de/AUTH.md) | [Français](../fr/AUTH.md) | [Español](../es/AUTH.md) | [Polski](AUTH.md) | [日本語](../ja/AUTH.md) | [简体中文](../zh/AUTH.md)

`dbwarp-blueprint` obsługuje tryby uwierzytelniania najczęściej potrzebne podczas zbierania Blueprint z PostgreSQL, MySQL i SQL Server.

## Nazwa użytkownika

Nazwę użytkownika można podać w URI albo oddzielnie:

```bash
--connect postgresql://app@db.internal/payments
```

lub:

```bash
--connect postgresql://db.internal/payments --user app
```

W przypadku nazw użytkowników trudnych do zakodowania w URI użyj:

```bash
--user-file /path/to/user.txt
--user-env DB_USER
```

## Hasło

Zalecane:

```bash
--password-file /path/to/password.txt
```

Alternatywa:

```bash
--password-env DB_PASSWORD
```

Jeżeli nie podano źródła hasła, narzędzie w miarę możliwości wyświetli interaktywny monit.

Hasła osadzone w URI połączenia są odrzucane.

## Token Entra ID dla SQL Server

W przypadku Azure SQL Database lub Managed Instance korzystających z Microsoft Entra ID wygeneruj token zwykłym narzędziem i przekaż go do `dbwarp-blueprint` jako sekret.

Plik tokenu:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-file /secure/path/token.txt \
  --tls-mode verify-full \
  --measure-compression --yes \
  --out blueprint.toml
```

Wskazana zmienna środowiskowa:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@server.database.windows.net,1433/db \
  --azure-token-env AZURE_SQL_TOKEN \
  --tls-mode verify-full \
  --out blueprint.toml
```

Narzędzie nie wywołuje Azure CLI, nie odświeża tokenów i nie zapisuje tokenu na dysku.

## Zintegrowane uwierzytelnianie SQL Server

Uwierzytelnianie zintegrowane korzysta z poświadczenia systemu operacyjnego, które jest już dostępne na hoście.

Linux Kerberos / GSSAPI:

```bash
kinit user@EXAMPLE.COM
DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh
./target/release/dbwarp-blueprint \
  --connect sqlserver://db.internal,1433/payments \
  --auth-mode integrated \
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' \
  --tls-mode verify-full \
  --out blueprint.toml
```

Windows SSPI:

```powershell
.\dbwarp-blueprint.exe `
  --connect sqlserver://db.internal,1433/payments `
  --auth-mode integrated `
  --expect-server-principal 'EXAMPLE\dbwarp-blueprint' `
  --tls-mode verify-full `
  --out blueprint.toml
```

W trybie `integrated` narzędzie `dbwarp-blueprint` nie odczytuje hasła. System operacyjny przekazuje token uwierzytelniający sterownikowi SQL Server.

Uwierzytelnianie zintegrowane jest dostępne wyłącznie dla SQL Server. PostgreSQL i MySQL odrzucają `--auth-mode integrated` z kodem `DBP1005E`.

Powyższe przykłady zakładają, że podmiot zabezpieczeń Windows już istnieje jako login SQL Server. Skrypty poziomów w `sql/grants/` tworzą login SQL z hasłem, co jest niewłaściwe dla tego trybu. Najpierw utwórz login za pomocą `FROM WINDOWS`, a następnie zastosuj bez zmian uprawnienia danego poziomu. Różni się tylko DDL loginu. Instrukcje oraz przypadki grup, zarządzanych kont usług i kont komputerów opisano w sekcji [Podmioty zabezpieczeń Windows i domeny dla uwierzytelniania zintegrowanego](../../sql/grants/DATABASE_PERMISSIONS.md#windows-and-domain-principals-for-integrated-authentication).

W tym trybie dwie kwestie operacyjne są ważniejsze niż przy `sql-auth`. Konto, na którym działa proces kolektora, jest tożsamością widzianą przez SQL Server. Jeżeli administrator uruchomi kolektor na hoście, na którym `BUILTIN\Administrators` należy do `sysadmin`, sesja działa jako `sysadmin` i omija każdą regułę `DENY` w skrypcie uprawnień, mimo że przechwytywanie kończy się powodzeniem. Opcja `--expect-server-principal` powoduje w takim przypadku błąd `DBP1606E` przed odczytem katalogu. Ponadto dedykowane konto usługi nie dziedziczy dostępu do plików po osobie, która je uruchomiła. Potrzebuje odczytu własnego pliku poświadczeń, jeżeli taki plik jest używany, oraz zapisu w ścieżkach `--out` i `--audit-log`.

Każde połączenie z SQL Server zapisuje `ORIGINAL_LOGIN()`, `SUSER_SNAME()` i
`USER_NAME()` w lokalnym audycie. Opcja `--expect-server-principal` jest
opcjonalna i działa także z uwierzytelnianiem SQL. SQL Server porównuje wtedy
`ORIGINAL_LOGIN()` z oczekiwanym podmiotem zabezpieczeń w ustanowionej sesji.
Niezgodność lub niedostępna tożsamość powoduje `DBP1606E` przed odczytem
katalogu. Dokładne tożsamości pozostają lokalnym dowodem audytowym i nie trafiają
do Blueprint, prezentacji ani artefaktów publikacyjnych.

## Uwierzytelnianie baz danych zarządzanych w chmurze

Zarządzany punkt końcowy sam w sobie nie zmienia uprawnień bazy danych wymaganych przez `dbwarp-blueprint`. Natywna nazwa użytkownika i hasło używają `sql-auth` i nie wymagają roli płaszczyzny sterowania chmurą po przygotowaniu sieci i konta bazy danych.

`dbwarp-blueprint` nie wywołuje interfejsów CLI chmury, usług metadanych, menedżerów sekretów ani interfejsów API odświeżania tokenów. Wrapper musi wygenerować lub pobrać każdy krótkotrwały token i przekazać go przez jedno chronione źródło sekretu.

### Tokeny chmurowe PostgreSQL i MySQL

Użyj `cloud-token` dla bezpośredniego tokenu zarządzanej usługi PostgreSQL lub MySQL wygenerowanego przez AWS, Azure lub Google Cloud. Podaj dokładnie jedną z opcji `--password-file` albo `--password-env`. Tryb wymaga `verify-full`; dodaj pakiet CA dostawcy lub instancji, jeśli nie jest zakotwiczony w zestawie zaufania wkompilowanym w plik binarny.

Przykład PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

Przykład MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://dbwarp_blueprint@managed-db.example.com/app \
  --auth-mode cloud-token \
  --password-file /secure/path/token.txt \
  --tls-mode verify-full --tls-ca /secure/path/provider-ca.pem \
  --out blueprint.toml --yes
```

Dla MySQL tryb `cloud-token` włącza wymianę `mysql_clear_password` tylko wewnątrz tej zweryfikowanej sesji TLS. W zwykłych połączeniach `sql-auth` wtyczka pozostaje wyłączona. PostgreSQL używa zwykłego protokołu hasła z tym samym wymaganiem zweryfikowanego TLS.

### Uprawnienia środowiska uruchomieniowego po stronie chmury

Te uprawnienia autoryzują logowanie lub tunel połączenia; nigdy nie zastępują podmiotu i uprawnień w bazie danych:

| Zarządzana ścieżka | Tryb pliku binarnego | Uprawnienie poza bazą danych w czasie wykonywania |
|---|---|---|
| Logowanie IAM do RDS/Aurora PostgreSQL lub MySQL | `cloud-token` | `rds-db:connect` dla dokładnego ARN użytkownika bazy danych |
| Logowanie Entra do Azure Database for PostgreSQL/MySQL | `cloud-token` | Brak roli RBAC zasobu Azure dla dostępu do danych; tożsamość musi być odwzorowana w bazie danych |
| Bezpośrednie logowanie IAM do Cloud SQL PostgreSQL/MySQL | `cloud-token` | Dokładne uprawnienie `cloudsql.instances.login`; `roles/cloudsql.instanceUser` jest szerszą predefiniowaną alternatywą |
| Cloud SQL Auth Proxy lub łącznik | Zwykle `sql-auth`; proxy może wykonywać automatyczne uwierzytelnianie IAM | Tożsamość proxy wymaga `roles/cloudsql.client`; automatyczne uwierzytelnianie IAM wymaga też uprawnienia logowania |
| Logowanie Entra do Azure SQL Database lub Managed Instance | `entra-token` | Brak roli RBAC zasobu Azure dla dostępu do danych; użyj opisanych wyżej opcji tokenu SQL Server |
| Każda obsługiwana zarządzana baza danych z natywnymi poświadczeniami | `sql-auth` | Brak |

Przegląd uprawnień wdrożenia powinien rejestrować zależne od wersji uprawnienia bazy danych, dokładne polityki chmurowe, alternatywy wbudowanych ról i zastrzeżenia dotyczące zakresu. Konfiguracja dostawcy, tworzenie podmiotów, dostęp sieciowy, generowanie tokenów i opcjonalne pobieranie sekretów są obowiązkami procesu przygotowania lub wrappera; nie są to uprawnienia, które należy przypisywać kolektorowi tylko dlatego, że punkt końcowy jest zarządzany.

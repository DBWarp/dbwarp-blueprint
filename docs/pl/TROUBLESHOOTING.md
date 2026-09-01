# Rozwiązywanie problemów

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../TROUBLESHOOTING.md).

[English](../TROUBLESHOOTING.md) | [Deutsch](../de/TROUBLESHOOTING.md) | [Français](../fr/TROUBLESHOOTING.md) | [Español](../es/TROUBLESHOOTING.md) | [Polski](TROUBLESHOOTING.md) | [日本語](../ja/TROUBLESHOOTING.md) | [简体中文](../zh/TROUBLESHOOTING.md)

Typowe awarie `dbwarp-blueprint` i zalecane dalsze działania.

Awarie pod kontrolą operatora zaczynają się teraz od stabilnego kodu komunikatu `DBPnnnnS`, na przykład `DBP1001E`.
Używaj tego kodu podczas przeszukiwania dokumentacji lub otwierania zgłoszenia do pomocy technicznej. Zobacz [Kody komunikatów operatorskich](MESSAGES.md).

## Żądany język nie jest używany

Podczas diagnozowania wyboru ustawień regionalnych użyj jawnej obsługiwanej wartości:

```bash
dbwarp-blueprint --lang pl --help
```

Obsługiwane wartości to `en`, `de`, `fr`, `es`, `pl`, `ja` i `zh`. Bez
`--lang` narzędzie sprawdza kolejno `DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES`
i `LANG`. Nieobsługiwana wartość jawna jest odrzucana z kodem `DBP1011E`;
niekompletny wbudowany katalog powoduje błąd uruchomienia `DBP1010E` zamiast
powrotu do języka angielskiego.

W Windows zmienne ustawień regionalnych zwykle nie są ustawione; podaj `--lang` albo ustaw `DBWARP_BLUEPRINT_LANG`.

## Nieprawidłowa szerokość lub kolory banera

Szerokość pochodzi z `COLUMNS`, jeśli ją ustawiono, w przeciwnym razie z konsoli w Linux i macOS, a następnie wynosi 80 kolumn. Obsługa kolorów zależy od `NO_COLOR`, `TERM` i `COLORTERM`; brak `TERM`, normalny w Windows, wybiera 16 kolorów. Użyj `--color always`, `--color never` albo ustaw `COLUMNS`.

## Hasło w URI jest odrzucane

Objaw:

```text
DBP1001E refusing to use URI-embedded password
```

Rozwiązanie: usuń hasło z URI i użyj jednej z opcji:

```bash
--password-file /path/to/pass
--password-env DBWARP_BLUEPRINT_PASSWORD
```

W systemie Unix tryb pliku nie może zezwalać na odczyt grupie ani innym użytkownikom.

## Błąd uprawnień pliku hasła

Objaw: narzędzie odrzuca `--password-file` lub `--tls-key`, ponieważ uprawnienia są zbyt szerokie.

Rozwiązanie:

```bash
chmod 600 /etc/dbwarp/db.pass
chmod 600 /etc/dbwarp/client.key
```

Zapobiega to przypadkowemu ujawnieniu danych innym lokalnym użytkownikom tego samego hosta.

## Weryfikacja TLS nie powiodła się

Użyj `--tls-mode verify-full` z właściwym pakietem CA i nazwą hosta:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

Jeżeli nazwa hosta w certyfikacie jest niezgodna, popraw nazwę DNS lub certyfikat. `--tls-skip-verify` jest odrzucane dla hostów innych niż pętla zwrotna, chyba że podano również `--i-know-what-im-doing`; nie używaj tej opcji w środowisku produkcyjnym.

## Korzenie zaufania TLS dla SQL Server

W trybach SQL Server weryfikujących certyfikat używany jest magazyn zaufania
systemu operacyjnego, gdy pominięto `--tls-ca`. Dostarczony plik `.pem` lub
`.crt` musi zawierać dokładnie jeden certyfikat CA i zastępuje te korzenie.
Sterownik sprawdza nazwę hosta połączenia zarówno w trybie `verify-ca`, jak i
`verify-full`.

## Poziom 2 wymaga zgody

Objaw:

```text
--measure-compression requires --yes
```

Rozwiązanie:

```bash
--measure-compression --yes
```

Jest to celowo jawne, ponieważ poziom 2 odczytuje ograniczone próbki wierszy do pamięci przed ich usunięciem.

## Próbkowanie trwa zbyt długo

Zmniejsz jedną lub obie wartości:

```bash
--sample-rows 500
--max-wall-secs 120
```

Podczas pierwszego przeglądu produkcyjnego mniejsza próbka poziomu 2 jest lepsza niż brak pomiaru kompresji. Jeżeli wyniki są obciążone lub niepełne, uruchom narzędzie ponownie na replice z większym budżetem czasu.

## DBA zabrania niekatalogowego zapytania SELECT 1

Wyłącz pomiar RTT:

```bash
--no-rtt-probe
```

Domyślny pomiar RTT składa się z pięciu zapytań `SELECT 1` i nie odczytuje danych wierszy, ale niektóre zasady uznają każde zapytanie niekatalogowe za wykraczające poza zakres.

## Dane wyjściowe nie zawierają sekcji kompresji

Sekcje kompresji pojawiają się tylko wtedy, gdy podano obie flagi:

```bash
--measure-compression --yes
```

Blueprinty wyłącznie katalogowe są prawidłowe, ale dalsze oszacowania kompresji będą wywnioskowane.

## Niektóre próbki kompresji są oznaczone jako obciążone

Niektóre silniki nie zapewniają jednolitego próbkowania tabel we wszystkich przypadkach, a małe tabele mogą wymagać mechanizmu awaryjnego `LIMIT`. Plik Blueprint zapisuje `sampled_with_bias` oraz `bias_reason`, aby estymator i recenzent mogli to uwzględnić.

Obciążone próbki nadal są użyteczne; mają jedynie mniejszą wartość niż próbki jednolite.

## Generowanie prezentacji z TOML nie powiodło się

`--from-toml` musi być użyte razem z `--deck`:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

Nie dołączaj flag bazy danych na żywo do `--from-toml`. Narzędzie odrzuca mieszane tryby na żywo/offline, aby zachować prostą granicę audytu.

## Plik Blueprint wygląda na zbyt mały

Zwykły plik Blueprint jest niewielki. Zawiera metadane strukturalne, zaokrąglone liczby, indeksy, strukturę grafu FK oraz opcjonalne podsumowania kompresji. Nie powinien zawierać wartości wierszy ani identyfikatorów.

Jeżeli potrzebujesz reprezentatywnej bazy danych do benchmarku, przekaż zatwierdzony plik `blueprint.toml` do osobno zweryfikowanych narzędzi dalszego etapu, autoryzowanych dla tego zadania.

## Trzeba udowodnić, że nie nastąpiło przesyłanie danych

Użyj dziennika audytu i narzędzi sieciowych:

```bash
./dbwarp-blueprint ... --audit-log audit.txt
strace -f -e trace=connect ./dbwarp-blueprint ...
tcpdump host db.internal
```

Oczekiwane zachowanie sieciowe podczas działania zależy od aktywnego trybu. Uruchomienie na żywo z `--connect` otwiera żądaną sesję bazy danych; DNS może kontaktować się ze skonfigurowanym resolwerem, a zintegrowane uwierzytelnianie Kerberos/SSPI może kontaktować się z KDC lub kontrolerem domeny. Tryb wsadowy otwiera po jednej sesji bazy danych dla każdego źródła bazodanowego. Lokalne operacje na TOML, Parquet, Avro i pakietach nie inicjują połączenia sieciowego aplikacji, chociaż ścieżki montowane z sieci podlegają zachowaniu stosu pamięci masowej hosta.

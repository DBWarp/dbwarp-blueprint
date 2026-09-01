# Model bezpieczeństwa

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../SECURITY.md).

[English](../../SECURITY.md) | [Deutsch](../de/SECURITY.md) | [Français](../fr/SECURITY.md) | [Español](../es/SECURITY.md) | [Polski](SECURITY.md) | [日本語](../ja/SECURITY.md) | [简体中文](../zh/SECURITY.md)

`dbwarp-blueprint` udostępnia odrębne tryby pracy z bazą danych na żywo,
plikami strukturalnymi, przetwarzaniem wsadowym, pakietami i prezentacjami.
Wybrany tryb określa jego zakres dostępu do sieci i systemu plików. Narzędzie
nie ma telemetrii, sprawdzania aktualizacji, sprawdzania licencji, wywołań
analitycznych ani ścieżki przesyłania danych.

Na tej stronie opisano granice bezpieczeństwa, aby zespół mógł zdecydować, czy uruchomić narzędzie.

## Zgłaszanie podatności

Podejrzewane podatności zgłaszaj prywatnie przez
[prywatne zgłaszanie podatności w GitHub](https://github.com/DBWarp/dbwarp-blueprint/security/advisories/new).
Nie umieszczaj informacji wrażliwych dla bezpieczeństwa w publicznym
zgłoszeniu. Podaj dokładną wersję wydania, system operacyjny, kroki odtworzenia
i najmniejszy bezpieczny zestaw dowodów potrzebny do oceny zgłoszenia.

## Sieć

| Tryb | Wykorzystanie sieci podczas działania |
|---|---|
| Baza danych na żywo `--connect` | Jedna sesja sterownika bazy danych ze wskazanym punktem końcowym. Rozwiązywanie nazw DNS może kontaktować się ze skonfigurowanym resolverem. Zintegrowane uwierzytelnianie Kerberos/SSPI może także kontaktować się ze skonfigurowaną infrastrukturą tożsamości, na przykład KDC lub kontrolerem domeny. |
| `--batch-manifest` | Jedna sesja sterownika bazy danych dla każdego źródła bazodanowego w manifeście, przetwarzana sekwencyjnie. Lokalne źródła Parquet i Avro nie używają sieci. Nadal obowiązują powyższe zastrzeżenia dotyczące DNS i uwierzytelniania zintegrowanego. |
| `--from-toml`, `--from-parquet`, `--from-avro`, `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Brak połączeń sieciowych inicjowanych przez aplikację. Dane wejściowe na sieciowych systemach plików pozostają kwestią systemu operacyjnego lub warstwy pamięci masowej. |

Narzędzie nie wywołuje usług DBWarp ani interfejsów API chmury. Sterowniki baz
danych i system operacyjny hosta mogą generować opisany powyżej ruch pomocniczy.

`--max-wall-secs` ustanawia dwa niezależne zabezpieczenia. PostgreSQL używa
lokalnego dla sesji `statement_timeout`, a MySQL lokalnego dla sesji
`max_execution_time` dla instrukcji `SELECT` kolektora przeznaczonych wyłącznie
do odczytu. SQL Server nie ma równoważnego ustawienia sesji ograniczającego
całkowity czas trwania instrukcji, dlatego kolektor ustawia lokalny dla sesji
`LOCK_TIMEOUT`, aby ograniczyć oczekiwanie na blokady, i zachowuje termin po
stronie klienta dla pozostałych zastojów. Jeśli termin klienta upłynie,
narzędzie zrywa połączenie; nie twierdzi, że SQL Server potwierdził anulowanie
po stronie serwera. Przed ponowieniem upewnij się, że praca serwera została
zatrzymana.

## Odczytywane pliki

Podczas działania narzędzie odczytuje tylko dane wejściowe wybrane w wierszu
poleceń lub wskazane przez dane wejściowe przetwarzania wsadowego albo pakietu:

| Plik | Kiedy |
|---|---|
| `--user-file` | źródło nazwy użytkownika |
| `--password-file` | źródło hasła |
| `--anonymization-key-file` | opcjonalny klucz HMAC przechowywany przez klienta, używany do zachowania anonimowych etykiet obiektów między zatwierdzonymi uruchomieniami; w systemie Unix tryb pliku nie może zezwalać na odczyt grupie ani innym użytkownikom |
| `--azure-token-file` | źródło tokenu Entra ID dla SQL Server |
| `--tls-ca` | zaufany pakiet CA |
| `--tls-cert` | certyfikat TLS klienta |
| `--tls-key` | prywatny klucz TLS klienta |
| `--from-toml` | istniejący plik TOML dbwarp-blueprint używany do utworzenia prezentacji offline |
| `--from-parquet` | metadane pliku Parquet oraz, po jawnej zgodzie na próbkowanie, ograniczoną liczbę zdekodowanych wierszy |
| `--from-avro` | metadane kontenera obiektów Avro i rekordy; Avro musi zostać odczytane sekwencyjnie, aby policzyć rekordy |
| `--batch-manifest` | manifest wsadowy oraz każdy lokalny plik strukturalny, plik poświadczeń, plik tokenu i plik TLS, do którego się odwołuje |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | plik TOML pakietu i wszystkie względne pliki Blueprint wymagane przez wybraną operację |
| `/dev/tty` | interaktywny monit o hasło w systemach uniksopodobnych |

Narzędzie nie odczytuje `~/.pgpass`, `~/.my.cnf`, plików poświadczeń chmurowych, kluczy SSH, historii powłoki ani domyślnych zmiennych środowiskowych haseł.

W przypadku PostgreSQL i MySQL dostarczony pakiet PEM `--tls-ca` zastępuje
wkompilowane korzenie Mozilla. SQL Server używa magazynu zaufania systemu
operacyjnego, gdy pominięto `--tls-ca`; dostarczony plik `.pem` lub `.crt` musi
zawierać dokładnie jeden certyfikat CA i zastępuje te korzenie. SQL Server
sprawdza nazwę hosta w obu trybach weryfikujących certyfikat i odrzuca
`--tls-cert`/`--tls-key` z kodem `DBP1015E`, ponieważ jego sterownik nie
implementuje uwierzytelniania certyfikatem klienta.

## Zapisywane pliki

Podczas działania narzędzie zapisuje:

| Plik | Kiedy |
|---|---|
| `--out` | dane wyjściowe Blueprint w trybie bazy danych na żywo, pliku strukturalnego, rozpakowania pakietu lub pakowania pakietu |
| `--deck` | opcjonalne podsumowanie PowerPoint (.pptx), generowane lokalnie z zanonimizowanego Blueprint lub danych wejściowych `--from-toml` (bez dodatkowego odczytu bazy danych, bez sieci i bez biblioteki zewnętrznej) |
| `--audit-log` | opcjonalna kopia dziennika audytu |
| `--out-dir` | katalog wsadowy zawierający `bundle.toml`, `blueprints/*.blueprint.toml`, `audits/*.audit.txt`, znacznik właściciela i `errors.txt`, gdy co najmniej jedno źródło zawiedzie; podczas publikacji atomowej używany jest katalog tymczasowy obok katalogu docelowego, usuwany po obsłużonej awarii |

Dziennik audytu jest również wypisywany na stderr.

Traktuj każdy dziennik audytu i wsadowy plik `errors.txt` jako dowód operacyjny
z kontrolą dostępu. Mogą one zawierać nazwy punktów końcowych, lokalne ścieżki,
identyfikatory źródeł z manifestu, błędy sterowników i dane czasowe. Dla SQL
Server dziennik audytu zawiera dokładny uwierzytelniony login
(`ORIGINAL_LOGIN()`), efektywny podmiot serwera (`SUSER_SNAME()`) i podmiot
bazy danych (`USER_NAME()`), a także opcjonalny oczekiwany podmiot i wynik
asercji. Te tożsamości nie są zapisywane do Blueprint ani prezentacji
pochodzących z jednego źródła. Metadane pakietu zachowują identyfikatory źródeł,
tagi i identyfikatory grup zbiorów danych dostarczone przez operatora, dlatego
używaj anonimowych wartości i sprawdź plik TOML pakietu przed przekazaniem.

## Zmienne środowiskowe

Domyślnie w czasie działania nie są odczytywane żadne zmienne środowiskowe zawierające poświadczenia.

Jeżeli przekażesz `--password-env NAME`, `--user-env NAME` lub `--azure-token-env NAME`, narzędzie odczyta dokładnie tę wskazaną zmienną. Nie korzysta z typowych wartości domyślnych, takich jak `PGPASSWORD`, `MYSQL_PWD` czy `MSSQL_PASSWORD`.

## Poświadczenia

Poświadczenia są opakowane w typ `Secret`, który celowo nie implementuje `Debug`, `Display`, `Clone` ani serializacji. Utrudnia to skompilowanie kodu, który przypadkowo zapisuje je w logach.

Poświadczenia są przekazywane sterownikowi bazy danych wyłącznie podczas zestawiania połączenia. Nie są zapisywane w pliku wyjściowym ani w dzienniku audytu. Dziennik audytu zapisuje źródło poświadczenia, na przykład `file:/etc/dbwarp/db.pass`, a nie jego wartość.

## Odrzucane wzorce poświadczeń

Hasła osadzone w URI połączenia są odrzucane. Na przykład poniższy adres nie jest akceptowany:

```text
postgresql://user:password@host/db
```

Zamiast tego użyj `--password-file`, `--password-env` albo interaktywnego monitu. Zapobiega to ujawnieniu hasła w historii powłoki, liście procesów lub przewiniętym buforze terminala.

## Bezpieczeństwo danych wyjściowych

Plik Blueprint zaprojektowano tak, aby był czytelny dla człowieka i możliwy do sprawdzenia:

- rzeczywiste identyfikatory są zastępowane anonimowymi nazwami chronionymi kluczem, takimi jak `table-001` i `col-1`
- wartości liczbowe są zaokrąglane do udokumentowanych przedziałów
- komentarze są stałe i nie służą jako kanał danych
- wartości wierszy nigdy nie trafiają do danych wyjściowych
- próbki kompresji, jeśli są włączone, są kompresowane lokalnie i usuwane

Poziom 2 na żywo stosuje sztywny limit 16 MiB projektowanego ładunku na tabelę,
zanim sterownik bazy danych otrzyma dane wierszy. Dla wyjątkowo szerokich tabel
zmniejsza żądaną liczbę wierszy, a komórki o zmiennej szerokości projektuje z
użyciem natywnego dla silnika obcinania po stronie serwera. Sondy stylu są
ograniczane oddzielnie w projekcji SQL. Lokalny koder ramek wierszy niezależnie
wymusza ten sam limit tabeli. Zapobiega to przesyłaniu nieograniczonego ładunku
LOB przy małej wartości `--sample-rows`; oznacza to również, że bardzo duże
wartości wpływają na oszacowania kompresji i długości wyłącznie przez swoje
ograniczone prefiksy.

Kolejność tabel, schematów, indeksów i obiektów innych niż tabele wykorzystuje
HMAC-SHA256 z separacją domen. Domyślnie narzędzie pobiera z systemu operacyjnego
nowy klucz lokalny dla procesu i nigdy go nie emituje, co uniemożliwia
czytelnikowi offline sprawdzanie potencjalnych nazw źródłowych. Używaj
`--anonymization-key-file` tylko wtedy, gdy te same anonimowe etykiety muszą
zostać zachowane między zatwierdzonymi uruchomieniami porównawczymi. Plik musi
zawierać dokładnie 32 surowe bajty lub 64 znaki szesnastkowe i musi być chroniony
jak poświadczenie. Audyt zapisuje jedynie, czy użyto klucza efemerycznego, czy
przechowywanego przez klienta; nigdy nie zapisuje wartości klucza.

Zmniejsza to ryzyko ujawnienia, ale nie sprawia, że każdy plik wyjściowy jest
bezpieczny dla każdego odbiorcy. Anonimowy kształt schematu, grafy zależności,
wersje silników, dokładne pola opcjonalne i nietypowe rozkłady rozmiarów mogą
umożliwiać rozpoznanie obciążenia. Przed udostępnieniem sprawdź dane wyjściowe
Blueprint i pakietów zgodnie z polityką klasyfikacji danych swojej organizacji.
Nie wysyłaj dzienników audytu ani `errors.txt` tak, jakby były zanonimizowanymi
plikami Blueprint.

Dokładny opis pól znajduje się w [`FORMAT.md`](FORMAT.md).

## Dziennik audytu

Każde uruchomienie generuje dziennik audytu zawierający:

- punkt końcowy bazy danych, z którym nawiązano połączenie
- użyte źródło poświadczeń
- uwierzytelniony i efektywny podmiot SQL Server oraz podmiot bazy danych, gdy
  sesja może je zgłosić
- tryb TLS
- odczytane pliki
- zapisane pliki
- wykonane zapytania
- informację, czy włączono próbkowanie wierszy
- wynik końcowy

Zobacz [`AUDIT.md`](AUDIT.md).

## Punkty wyjścia do przeglądu kodu źródłowego

Do ukierunkowanego przeglądu służą:

- `src/secret.rs`: opakowanie poświadczeń
- `src/main.rs`: CLI, bramki zgody i emisja audytu
- `src/audit.rs`: renderowanie dziennika audytu
- `src/format.rs`: format zanonimizowanych danych wyjściowych
- `src/tls.rs`: konfiguracja TLS
- `src/engine_pg.rs`, `src/engine_mysql.rs`, `src/engine_mssql.rs`: czytniki katalogów właściwe dla baz danych

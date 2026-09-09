# Co odczytuje i zapisuje dbwarp-blueprint

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../AUDIT.md).

[English](../../AUDIT.md) | [Deutsch](../de/AUDIT.md) | [Français](../fr/AUDIT.md) | [Español](../es/AUDIT.md) | [Polski](AUDIT.md) | [日本語](../ja/AUDIT.md) | [简体中文](../zh/AUDIT.md)

Ten dokument opisuje zachowanie aplikacji podczas działania w zakresie sieci,
systemu plików, środowiska, bazy danych i audytu. Porównaj każdy aktywny tryb i
opcję z zasadami bezpieczeństwa swojej organizacji. Sterowniki baz danych,
biblioteki TLS i tożsamości, dynamiczny loader oraz system operacyjny mogą
dodatkowo korzystać z konfiguracji platformy, magazynów zaufania, DNS, pamięci
podręcznej poświadczeń i pamięci sieciowej; działania tej warstwy wspierającej
nie są w pełni widoczne w audycie aplikacji.

## Ruch wychodzący z sieci

Tryb `--connect` na żywo otwiera jedną sesję sterownika bazy danych ze wskazanym
punktem końcowym. Tryb wsadowy przetwarza źródła sekwencyjnie i otwiera jedną
sesję dla każdego źródła bazodanowego. Rozwiązywanie nazw DNS może korzystać ze
skonfigurowanego resolvera, a zintegrowane uwierzytelnianie Kerberos/SSPI może
kontaktować się z KDC lub kontrolerem domeny. Operacje offline na plikach TOML,
Parquet, Avro i pakietach nie otwierają połączeń sieciowych inicjowanych przez
aplikację, choć ścieżka na sieciowym systemie plików nadal podlega stosowi
pamięci masowej hosta.

Plik binarny nie zawiera telemetrii, sprawdzania licencji, aktualizacji wersji,
wywołań API chmury ani ścieżki przesyłania danych.

Możesz to zweryfikować za pomocą `strace -f -e trace=connect,sendto,recvfrom`,
`tcpdump` lub eBPF na wybranej platformie.

## Odczyty systemu plików

Narzędzie odczytuje dane wejściowe wybrane przez aktywny tryb:

| Plik | Kiedy | Zawartość |
|---|---|---|
| `--user-file PATH` | Jeśli podano | Tylko nazwa użytkownika. Końcowe białe znaki są usuwane; pusty plik jest błędem. |
| `--password-file PATH` | Jeśli podano | Odczytywany raz. Bufor `Secret` należący do DBWarp jest zerowany przy usunięciu; sterowniki mogą zachować własne kopie zgodnie z opisem w SECURITY.md. Odrzucany w systemach Unix, jeśli może go odczytać grupa lub inni. |
| `--anonymization-key-file PATH` | Jeśli podano | Przechowywany przez klienta klucz HMAC o długości 32 bajtów lub 64 znaków szesnastkowych. Odrzucany w systemach Unix, jeśli może go odczytać grupa lub inni. Klucz nigdy nie jest emitowany. |
| `--azure-token-file PATH` | Jeśli podano | Token Entra ID dla SQL Server. Odczytywany raz; bufor `Secret` należący do DBWarp jest zerowany przy usunięciu. Odrzucany w systemach Unix, jeśli może go odczytać grupa lub inni. |
| `--tls-ca PATH` | Jeśli podano | Zaufany CA PEM odczytywany podczas łączenia. PostgreSQL/MySQL akceptują pakiet; SQL Server akceptuje dokładnie jeden certyfikat. Dostarczony plik zastępuje domyślne korzenie silnika. |
| `--tls-cert PATH` | Jeśli podano | Certyfikat TLS klienta PostgreSQL/MySQL (PEM), odczytywany podczas łączenia. Odrzucany dla SQL Server z kodem `DBP1015E`. |
| `--tls-key PATH` | Jeśli podano | Klucz TLS klienta PostgreSQL/MySQL (PEM). W systemach Unix odrzucany, jeśli tryb pozwala na odczyt grupie lub innym użytkownikom. Odczytywany podczas łączenia; dla SQL Server odrzucany z kodem `DBP1015E`. |
| `--from-toml PATH` | Jeśli podano | Istniejący plik TOML dbwarp-blueprint, odczytywany lokalnie w celu zbudowania prezentacji bez połączenia z bazą danych. |
| `--from-parquet PATH` | Jeśli podano | Metadane Parquet oraz, wyłącznie po jawnej zgodzie na próbkowanie, ograniczoną liczbę zdekodowanych wierszy. |
| `--from-avro PATH` | Jeśli podano | Metadane kontenera Avro i rekordy; kontener jest odczytywany sekwencyjnie w celu uzyskania liczby wierszy. |
| `--batch-manifest PATH` | Jeśli podano | Manifest oraz każde wskazane przez niego lokalne wejście, poświadczenie, token i ścieżkę TLS. |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | Jeśli podano | Plik TOML pakietu oraz względne pliki Blueprint wymagane do wyświetlania, rozpakowania lub pakowania. |
| terminal/konsola sterująca (`/dev/tty` w systemach uniksowych) | Jeśli nie podano źródła hasła | Monit z wyłączonym echem. |
| (tylko podczas budowania) `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.dbwarp-source-revision` w wydaniach vendored, `vendor/mysql_async`, `vendor-crates/*` w pakietach offline | Tylko gdy uruchomiono `./build.sh` | Toolchain, pochodzenie źródła i standardowe dane wejściowe Cargo |

Aplikacja nie ma jawnej ścieżki odczytu:
- `~/.pgpass`, `~/.my.cnf`, `~/.aws/credentials`, `~/.azure/credentials`
- żadnych plików `~/.ssh/*`
- `/etc/passwd` ani `/etc/shadow` jako wejść Blueprint (biblioteki tożsamości i
  uwierzytelniania platformy mogą jednak korzystać z danych kont systemowych)
- żadnej zmiennej poświadczeń bazy poza wskazaną przez `--password-env`,
  `--user-env` lub `--azure-token-env`. Kompilacje ze zintegrowanym Kerberos
  mogą również spowodować, że stos GSSAPI/Kerberos platformy skorzysta z własnej
  konfiguracji, pamięci podręcznej, keytabu i zmiennych środowiskowych. Zmienne
  języka i prezentacji terminala opisano poniżej.

## Zapisy systemu plików

Narzędzie zapisuje tylko dane wyjściowe wybrane przez aktywny tryb:

| Plik | Kiedy | Zawartość |
|---|---|---|
| `--out PATH` (domyślnie `./blueprint.toml`) | Uruchomienia bazy danych na żywo, Parquet, Avro, bundle-extract i bundle-pack | Plik TOML Blueprint lub spakowanego pakietu. Nie jest zapisywany w trybie samej prezentacji, bundle-list, próbnym ani pomocy/wersji. |
| `--deck PATH` | Tylko jeśli podano | Prezentacja PowerPoint (.pptx) podsumowująca zanonimizowany Blueprint. Budowana lokalnie z tego samego Blueprint w pamięci albo z danych wejściowych `--from-toml` — bez dodatkowego odczytu bazy danych, sieci i biblioteki zewnętrznej. |
| `--audit-log PATH` | Tylko jeśli podano | Atomowo zastępowana kopia dziennika audytu ze stderr; istniejąca treść nie jest dopisywana. |
| `--out-dir DIR` | Tryb wsadowy inny niż próbny | `bundle.toml`, osobne dla źródeł katalogi `blueprints/` i `audits/`, znacznik właściciela oraz `errors.txt` po częściowej awarii. Publikacja używa katalogu tymczasowego obok katalogu docelowego i znacznika odzyskiwania. |
| (tylko podczas budowania) `./target/`, `./build/` | Tylko gdy uruchomiono `./build.sh` | Standardowe dane wyjściowe budowania Cargo |

Aplikacja nie ma jawnej ścieżki zapisu do:
- `/var/log/*`
- `~/.cache/*`, `~/.local/*`, `~/.config/*`
- niejawnego systemowego katalogu tymczasowego (użytkownik nadal może jawnie
  wskazać tam plik wyjściowy lub katalog wsadowy)

## Odczytywane zmienne środowiskowe

Audyt wymienia tylko zmienne odczytywane przez sam DBWarp Blueprint. Jeśli `--lang` nie wybiera
obsługiwanego języka, wybór może odczytać `DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES` i `LANG`, w tej kolejności.
Prezentacja terminala może odczytać `NO_COLOR`, `TERM`, `COLORTERM` i `COLUMNS`;
wpływają one wyłącznie na prezentację.

Gdy podano `--password-env VAR_NAME` lub `--user-env VAR_NAME`, narzędzie
odczytuje dokładnie tę wskazaną zmienną. Nie ma mechanizmu powrotu do typowych
wartości domyślnych, takich jak `PGPASSWORD`, `MYSQL_PWD`, `MSSQL_PASSWORD`,
`USER` czy `LOGNAME` — te mechanizmy celowo nie zostały zaimplementowane.

Biblioteki platformy odpowiedzialne za bazę danych, TLS, DNS i zintegrowane
uwierzytelnianie mogą korzystać z własnych zmiennych i konfiguracji spoza tej
listy aplikacji. Jeśli zasady wymagają pełnego wykazu procesu i bibliotek, użyj
śledzenia na poziomie systemu operacyjnego.

Gdy uruchomiono `./build.sh`, odczytywane są `PINNED_RUST` (nadpisanie),
`ALLOW_NETWORK` (zgoda na pobranie rustup-init), `TARGET` (cel kompilacji
krzyżowej) oraz standardowe zmienne Cargo/rustup. Żadna z nich nie jest
odczytywana przez samo narzędzie podczas działania.

## Dziennik audytu każdego uruchomienia

Przy każdym uruchomieniu narzędzie emituje dziennik audytu na stderr w stabilnym
układzie zwykłego tekstu. Przekieruj go do pliku przez `2>audit.txt` albo
użyj `--audit-log PATH`, aby utworzyć jawną kopię.

Przykład (poziom 1):

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
anonymization_key:   ephemeral-random
schema_selector_count: 1

connection:
  - postgresql://app@db.example:5432/payments
    auth: scram-sha-256-or-md5
    tls: yes (protocol version unavailable from driver)
    tls_ca_only: false

auth:
  user_source:        file:/etc/dbwarp/db.user
  password_source:    file:/etc/dbwarp/db.pass (mode 0o600)
  password_persisted: false
  password_logged:    false
  authenticated_principal: (not observed)
  effective_server_principal: (not observed)
  database_principal: (not observed)
  expected_server_principal: (not requested)
  principal_assertion: not-observed

topology_and_scope:
  topology:
    deployment: unknown
    local_role: unknown
    visibility: partial
    member_count: 2
    identifiers_redacted: true
    role_counts: primary=1, secondary=1
    features: postgresql-streaming-replication
    catalogs_read: pg-is-in-recovery, pg-stat-replication
    catalogs_unreadable: (none)
  dataset_scope:
    layout: full-copy
    table_inventory_completeness: complete
    row_count_completeness: complete
    size_completeness: complete
    row_count_method: postgres-planner-estimate
    size_method: postgres-local-relation-size
    limitations: row-counts-statistical

blueprint_fidelity_estimate:
  basis: evidence-coverage-v1
  overall_score: 79/100
  band: good
  structure_score: 90/100
  sizing_score: 100/100
  column_statistics_score: 68/100
  relationship_score: 75/100
  artifact_score: 50/100
  limitations: biased-column-sampling, cardinality-lower-bounds
  qualification: evidence estimate, not source-truth accuracy or a confidence interval

artifact_inventory:
  detail: summary
  visibility: full
  objects: 42
  dependency_edges: 0
  external_prerequisites: 3
  inventory_complete: false
  dependencies_complete: false
  analysis_complete: false

database_operations_observed:
  1. [succeeded, 14ms, 28 rows]   server version lookup
  2. [succeeded, 9ms, 312 rows]   column catalog lookup
  ... (every observed catalog operation enumerated)

wire_bytes_observed:
  catalog_responses: unknown (driver does not expose wire-byte totals)
  row_data:          unknown (driver does not expose wire-byte totals)

local_sample_processing:
  encoded_rowframe_bytes: 0 B

sampling_work:
  compression_workers: 0
  compression_queue_capacity: 0
  compression_jobs_submitted: 0
  compression_jobs_completed: 0
  compression_pipeline_wall_ms: 0
  compression_worker_ms: 0
  tables_skipped_proven_empty: 0
  chunk_level_3_attempts: 0
  table_level_3_attempts: 0
  column_level_3_attempts: 0

files_read_local:
  - /etc/dbwarp/db.pass        (mode 0o600 ✓)

files_written_local:
  - ./blueprint.toml         (12 KiB, sha256: 7f3e2af1...)

warnings:
  - (none)

network_egress:
  - db.example:5432 (database-driver session; DNS may use the configured resolver)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - length policy balanced: declared capacities and index prefixes exact; sampled lengths relatively rounded
  - identifier ordering uses domain-separated HMAC-SHA256 with a fresh process-local key; labels intentionally vary between runs
  - the anonymization key and source identifiers are not written to the Blueprint
  - artifact summary stores bounded counts and external-prerequisite classes; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential entered through the Secret wrapper and its buffer is zeroized on drop; driver APIs may retain copies as documented under 'Driver-owned credential copies' in SECURITY.md

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

Uruchomienia MySQL emitują właściwe dla trybu stwierdzenie `length policy balanced|strict|exact`.
Niezależnie określa ono, czy długości strukturalne i próbkowane są dokładne czy
zaokrąglone, dzięki czemu audyt nigdy nie twierdzi, że wszystkie wartości
liczbowe zostały zaokrąglone dla uruchomienia `balanced` lub `exact`.

Dziennik audytu:

- zapisuje wyłącznie liczbę powtarzalnych selektorów trybu na żywo `--schema`; ich wartości są pokazywane w interaktywnym podglądzie, ale nie są dodawane do audytu. Istniejący URI połączenia, z ukrytymi danymi wrażliwymi, nadal identyfikuje połączoną bazę danych, która w MySQL jest również nazwą schematu. Wybrany Blueprint ma oznaczenie `selection-limited` w `dataset_scope`;
- wskazuje rewizję źródła osadzoną podczas kompilacji i stan drzewa roboczego; końcowy SHA-256 pliku binarnego pozostaje zewnętrzną sumą wydania/rejestru, ponieważ plik nie może zawierać własnego końcowego skrótu;
- zapisuje **źródło** poświadczenia (ścieżkę pliku, nazwę zmiennej środowiskowej,
  TTY), nigdy jego wartość;
- dla SQL Server zapisuje dokładne tożsamości sesji zwracane przez
  `ORIGINAL_LOGIN()`, `SUSER_SNAME()` i `USER_NAME()`. Po podaniu
  `--expect-server-principal` zapisuje również oczekiwaną wartość i wynik
  porównania po stronie serwera przed odczytem katalogu;
- wymienia każdą zaobserwowaną operację bazy z wynikiem, czasem i liczbą wierszy, jeśli sterownik ją podał; końcowe błędy mają ograniczoną etykietę bez identyfikatorów;
- podaje bajty sieciowe jako `unknown`, gdy sterownik ich nie ujawnia, oraz osobno lokalnie zakodowane bajty próbki;
- zgłasza łączną liczbę bajtów zapisanych lokalnie (z sha256 każdego pliku);
- zapisuje niekrytyczne pogorszenia przechwytywania i próbkowania ze stabilnymi
  kodami ostrzeżeń DBP; pusta sekcja oznacza, że nie zaobserwowano znanego pogorszenia;
- kopiuje zweryfikowane dowody `[database_topology]` i `[dataset_scope]` do `topology_and_scope`, używając wyłącznie zamkniętych tokenów i liczników; nazwy węzłów, endpointy oraz identyfikatory klastra i bazy nie mogą się pojawić;
- zachowuje `DBP1411W`, `DBP1412W` i `DBP1413W`, gdy pokrycie topologii lub zbioru danych jest niepełne, aby udane przechwycenie nie ukrywało zastrzeżenia do wymiarowania;
- rejestruje deterministyczne, rozbite na wymiary oszacowanie wierności Blueprint. Wynik opisuje pokrycie zebranych dowodów dla struktury, wymiarowania, statystyk kolumn, relacji i artefaktów. Nie jest to zmierzony błąd względem danych źródłowych ani statystyczny przedział ufności. Statystyki PostgreSQL zgłoszone jako nieaktualne lub nigdy nieanalizowane obniżają wynik wymiaru dotyczącego rozmiaru i pojawiają się jako jawne ograniczenia `table-statistics-stale` lub `table-statistics-never-analyzed`. Aktualność statystyk nie może zrekompensować braku pokrycia liczby wierszy. Dla silników korzystających z liczników aktualizowanych na bieżąco lub takich, dla których nie można ustalić aktualności statystyk, nie wymyśla się dowodów ich aktualności;
- deklaruje zapewnienia zaufania właściwe dla trybu (poziom 1 lub poziom 2);
- używa stabilnego formatu tekstowego, ale wartości mogą zmieniać się wraz ze
  stanem bazy, czasem, ostrzeżeniami i domyślnie świeżym kluczem anonimizacji.
  Do zatwierdzonych porównań użyj ponownie chronionego
  `--anonymization-key-file` i ustal `--generated-at`; pola czasu nadal się różnią.

**Warunkowa emisja zapewnienia zaufania.** Wiersz
"credential entered through the Secret wrapper..." jest emitowany tylko podczas
uruchomień, w których rzeczywiście odczytano poświadczenie. Ścieżki błędów, które
kończą się przed pozyskaniem poświadczeń (błędy parsowania URI, odmowa przyjęcia
hasła osadzonego w URI, przebieg próbny itd.), celowo **nie** emitują tego
wiersza — nie ma czego zapewniać o poświadczeniu, którego nigdy nie pozyskano.
Obecność lub brak tego wiersza wraz z `auth.password_source` wskazuje, czy dane
uruchomienie wykonało obsługę poświadczeń.

**Audyt jest emitowany na operacyjnych ścieżkach powodzenia i błędu**, również
przy błędach parsowania wiersza poleceń po starcie. Wyjścia pomocy/wersji oraz
błędy sprzed załadowania wbudowanego kontraktu lokalizacji nie tworzą pełnego
audytu. Jeśli narzędzie zawiedzie w trakcie działania (odmowa uwierzytelnienia,
błąd sieci), dziennik audytu nadal jest wypisywany na stderr (oraz do
`--audit-log PATH`, jeśli podano) z wynikiem `outcome: error: <stage>`, dzięki
czemu klient zawsze ma kryminalistyczny zapis tego, co próbowano wykonać przed
awarią. Przykładowy wiersz wyniku awarii:

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

Dane wyjściowe terminala zawierają również zakodowane podsumowanie operatorskie,
takie jak `DBP1001E` lub `DBP0001E`, wraz z łańcuchem przyczyn. Wynik audytu ma
ograniczoną długość i może obcinać długi tekst; do segregacji zgłoszeń użyj
danych wyjściowych terminala oraz kodu komunikatu. Zobacz `MESSAGES.md`.

Opcjonalne pomiary RTT, kompresji i stylu tekstu mogą zakończyć się niepowodzeniem
bez unieważniania podstawowego przechwycenia katalogu. Takie przypadki są
wypisywane i zachowywane w sekcji `warnings:` jako `DBP1405W` do `DBP1408W`,
dzięki czemu udany, ale częściowy wynik poziomu 2 można odróżnić od wyniku
kompletnego. Powtarzające się identyczne ostrzeżenia są deduplikowane, a
wielowierszowe szczegóły sterownika spłaszczane, aby audyt był ograniczony i
możliwy do skanowania maszynowego.

## Odczyty artefaktów innych niż tabele

Zbieranie artefaktów jest niezależne od próbkowania wierszy poziomu 2:

- `--artifact-detail none` pomija katalogi artefaktów i definicje.
- `summary` odczytuje modelowane katalogi obiektów, ale nie tekst definicji.
- `graph` odczytuje dodatkowo katalogi zależności, ale nie tekst definicji.
- `analyzed` odczytuje dodatkowo dostępne definicje SQL/proceduralne do ograniczonej pamięci procesu na potrzeby analizy leksykalnej.

Audyt rejestruje żądany poziom szczegółowości, widoczność, liczniki obiektów, zależności i wymagań zewnętrznych oraz wszystkie flagi kompletności. Każda operacja katalogowa występuje w `database_operations_observed`. Nieudany opcjonalny katalog emituje `DBP1410W`, pojawia się w `warnings` i zapobiega nieprawdziwej deklaracji kompletności.

W trybie analizowanym definicje są przechowywane przez właściciela zerującego pamięć, czyszczone i redukowane do ograniczonych przedziałów i zamkniętych tokenów cech. Tekst definicji, nazwy obiektów źródłowych, zewnętrzne punkty końcowe, podmioty zabezpieczeń artefaktów, poświadczenia, materiał kluczy/certyfikatów, nazwy pakietów/bibliotek i pliki binarne nigdy nie są zapisywane w pliku Blueprint ani dzienniku audytu. Jedyne zachowane dokładne nazwy podmiotów to trzy tożsamości sesji SQL Server w wyraźnym bloku audytu `auth` opisanym powyżej; nigdy nie trafiają do Blueprint, prezentacji ani artefaktów publikacyjnych. Tryby graph i analyzed wymagają `--yes`, ponieważ anonimowa topologia może identyfikować aplikację.

Audyt rozróżnia postawy prywatności jedną z tych deklaracji zaufania:

- summary: tylko ograniczone liczniki, bez tożsamości obiektów i definicji;
- graph: anonimowy graf zależności, bez definicji;
- analyzed: definicje odczytane tymczasowo, zachowane tylko ograniczone przedziały cech.

Zobacz [`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md), aby poznać pokrycie rodzin obiektów i interpretację kompletności.

## Dodatki poziomu 2

Po akceptacji pomiaru interaktywnie albo bez interakcji przez `--measure-compression --yes` narzędzie dodatkowo:

- dla każdej tabeli, której pustości nie potwierdzono, wykonuje ograniczoną
  ścieżkę próbkowania właściwą dla silnika. PostgreSQL zaczyna od adaptacyjnego
  procentu dla `TABLESAMPLE SYSTEM`, wyliczanego z szacowanej liczby wierszy
  i żądanej wielkości próbki, z `LIMIT N`, a w razie potrzeby przechodzi do `LIMIT N`;
  MySQL używa czterech ograniczonych okien zakresu numerycznego klucza głównego,
  gdy taka bezpieczna ścieżka dostępu jest dostępna, a w przeciwnym razie
  `LIMIT N`; SQL Server używa `TOP N`. Ścieżki obciążone błędem ustawiają w
  danych wyjściowych `sampled_with_bias = true`;
- odczytuje próbkowane wiersze do lokalnego bufora w pamięci;
- utrzymuje sekwencyjny odczyt z bazy danych. Opcja
  `--compression-workers N` może uruchomić 1–32 ograniczonych lokalnych
  workerów kompresji (domyślnie 1, aby zminimalizować wpływ na host źródłowy).
  Zwiększ ją jawnie, aby użyć większej ilości lokalnego CPU. Każdy worker ma
  własne konteksty zstd, bez wspólnej blokady zstd;
- kompresuje przez zstd na poziomie 3;
- zapisuje wynikowe współczynniki i odchylenie standardowe;
- **usuwa każdy bufor po zakończeniu ograniczonego lokalnego zadania**. Bajty
  nie są zapisywane na dysku ani przesyłane. Pula przechowuje najwyżej N próbek
  w kolejce i N próbek aktualnie kompresowanych.

`local_sample_processing.encoded_rowframe_bytes` przedstawia bajty zakodowane
lokalnie do kompresji, a nie bajty sieciowe bazy. Bajty nieudostępniane przez
sterownik pozostają `unknown`. Blok `[compression]` dla poszczególnych tabel w
pliku wyjściowym zapisuje wartości współczynników. `--max-wall-secs` jest twardym limitem całego
przechwytywania na żywo, łącznie z połączeniem, katalogami, sondami RTT i
próbkowaniem Poziomu 2.
PostgreSQL ustawia również sesyjny `statement_timeout`; MySQL ustawia sesyjny
`max_execution_time` dla instrukcji `SELECT` przeznaczonych wyłącznie do
odczytu; SQL Server ustawia sesyjny `LOCK_TIMEOUT`, ponieważ nie ma równoważnego
limitu sesji dla czasu trwania instrukcji. Po upływie zewnętrznego terminu
klient zrywa połączenie. Audyt nie uznaje tego zerwania za dowód, że SQL Server
potwierdził anulowanie, dlatego operator musi przed ponowieniem potwierdzić
zatrzymanie pracy serwera.

`sampling_work` to pozbawiony identyfikatorów dowód operacyjny. Sekcja zapisuje
lokalne limity workerów i kolejki, limit 16 MiB projektowanego ładunku na tabelę,
zadania wysłane i zakończone, próby kompresji
oraz tabele pominięte w próbkowaniu, ponieważ katalog silnika potwierdził ich
pustość w chwili odczytu. `compression_worker_ms` jest łącznym czasem ściennym
workerów, a nie czasem CPU procesu, i może przekraczać
`compression_pipeline_wall_ms`, gdy workery działają równolegle. Czas ścienny
potoku może nakładać się na nadal sekwencyjne odczyty z bazy danych. Liczniki
opisują wykonaną pracę; nie są liczbami wierszy, pomiarami bajtów sieciowych ani
twierdzeniami o dokładności źródła.

## Protokół weryfikacji

Pobrane archiwum platformy i odtworzenie programu ze źródeł to dwie odrębne
kontrole zaufania. Najpierw zweryfikuj archiwum i plik wykonywalny za pomocą
sumy kontrolnej opublikowanej w tym samym wydaniu. Archiwum platformy jest
pakietem operatorskim, a nie drzewem źródeł: zawarte w nim dokumenty i
`verify.sh` są materiałem referencyjnym dla pasującego drzewa źródeł. Przed
audytem lub ponownym zbudowaniem pobierz dokładny tag wydania z publicznego
repozytorium albo użyj archiwum wydania ze źródłami i zależnościami. Archiwum
platformy nie jest przeznaczone do budowania programu w miejscu.

Jeżeli chcesz *udowodnić*, że narzędzie wykonuje wyłącznie udokumentowane działania:

1. **Integralność pobrania**: zweryfikuj archiwum platformy na podstawie
   właściwego wpisu w `SHA256SUMS.txt`, a następnie rozpakowany plik wykonywalny
   na podstawie jego pliku `*.binary.sha256`. Oba pliki sum kontrolnych muszą
   pochodzić z tego samego niezmiennego tagu wydania. Zobacz
   [Pobieranie plików binarnych](BINARIES.md).
2. **Audyt źródeł**: w pasującym drzewie źródeł lub archiwum ze źródłami i
   zależnościami przeczytaj `src/secret.rs`, a następnie wyszukaj
   `\.expose\(\)` poza tym plikiem. Jeżeli zainstalowano `rg`, krótka komenda to:
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   W przeciwnym razie użyj zatwierdzonego dla platformy narzędzia do
   rekurencyjnego przeszukiwania tekstu; `rg` nie jest wymagane do budowania ani
   weryfikacji.
   Produkcyjne miejsca wywołania natychmiast przekazują ujawnione `&str` do
   konstruktora połączenia. MySQL dodatkowo wywołuje `.to_string()`,
   ponieważ interfejs API `mysql_async` wymaga `String`. Ta niezerowana kopia
   trafia do opcji należących do sterownika i może przetrwać usunięcie
   `OptsBuilder`, pozostając przez czas życia opcji/połączenia. Usunięcie
   konstruktora nie dowodzi wymazania kopii. Tier 1 i Tier 2 używają tego
   samego połączenia MySQL. Pełne omówienie znajduje się w sekcji **Kopie
   poświadczeń należące do sterownika** w SECURITY.md.
3. **Budowanie ze źródeł**: `./build.sh`. Dla archiwum ze źródłami i
   zależnościami uruchom `DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh`. CI wydania
   wykonuje niezależną ponowną kompilację na tym samym runnerze, opróżniając
   i ponownie wykorzystując ten sam katalog docelowy Cargo między kompilacjami,
   a pierwszy plik wykonywalny zachowując osobno. Odrzuca każdą różnicę bajtów. Porównanie lokalne
   ma znaczenie tylko przy tej samej rewizji źródła, celu, zestawie funkcji,
   przypiętym toolchainie Rust, linkerze i flagach budowania.
4. **Porównanie z wydaniem**: w pasującym drzewie źródeł uruchom
   `./verify.sh /path/to/extracted/dbwarp-blueprint`. W sekcji **Odtwarzanie
   pliku binarnego wydania** w BUILD.md opisano wymagany cel, funkcje, toolchain,
   linker, epokę daty źródła i flagi budowania.
5. **Śledzenie działania**: w systemie Linux uruchom narzędzie przez
   `strace -f -e trace=open,connect,read,write` w piaskownicy. Jeżeli `strace`
   lub `rg` nie jest dostępne, użyj zatwierdzonych dla platformy odpowiedników
   do śledzenia plików/sieci i rekurencyjnego przeszukiwania tekstu. Porównaj
   wynik z powyższymi listami.
6. **Śledzenie sieci**: uruchom `tcpdump` na hoście. W uwierzytelnianym hasłem
   przebiegu na żywo sprawdź sesję bazy danych oraz oczekiwany ruch DNS. Dla
   uwierzytelniania zintegrowanego uwzględnij także oczekiwany ruch do KDC lub
   kontrolera domeny. W trybie wsadowym uzgodnij jedną sesję bazy danych na każde
   źródło bazodanowe.

Jeżeli cokolwiek nie odpowiada temu dokumentowi, zgłoś rozbieżność kanałem
wskazanym w SECURITY.md i dołącz tylko najmniejszy bezpieczny ślad potrzebny do
jej odtworzenia. Nie umieszczaj poświadczeń, identyfikatorów klienta ani
wrażliwych danych wyjściowych sterownika w publicznym zgłoszeniu.

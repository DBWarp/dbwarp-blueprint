# Format pliku DBWarp Blueprint, wersja 7.

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../FORMAT.md).

[English](../../FORMAT.md) | [Deutsch](../de/FORMAT.md) | [Français](../fr/FORMAT.md) | [Español](../es/FORMAT.md) | [Polski](FORMAT.md) | [日本語](../ja/FORMAT.md) | [简体中文](../zh/FORMAT.md)

Czytelny dla człowieka. Łatwy do porównywania. Możliwy do analizy
kryminalistycznej.

> **Ten format ogranicza ryzyko ukrytych kanałów i bezpośredniego ujawnienia
> dzięki ograniczonemu schematowi, identyfikatorom opartym na tajnym kluczu i
> udokumentowanej precyzji liczb. Anonimowa struktura grafu i dokładne pola
> opcjonalne nadal mogą identyfikować obciążenie, dlatego sprawdź plik zgodnie
> z własną polityką klasyfikacji danych.**

## Nagłówek pliku

Dosłownie, bajt po bajcie:

```
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

Pusty wiersz jest częścią kanonicznego nagłówka. Kolektor Rust emituje dokładnie
ten nagłówek i żadne inne komentarze. Normalizator zapasowej ścieżki SQL zachowuje
go dosłownie, a następnie dodaje jeden stały komentarz
`Producer: blueprint_format.py SQL fallback` ze źródłem klucza, aby odbiorcy mogli
rozróżnić producenta. Nie oznacza to jednak, że pozostałe pola strukturalne nie
mogą identyfikować charakterystycznego schematu lub grafu zależności.

## Pola najwyższego poziomu

| Pole | Typ | Opis |
|---|---|---|
| `schema_version` | int | Wersja formatu. Obecnie `7`. Wersje od 1 do 6 są nadal czytelne. |
| `generated_at` | Łańcuch znaków w formacie ISO-8601. | Znacznik czasu UTC, z dokładnością do sekund, bez części ułamkowych. Możliwość "ustawienia" (pinnable) za pomocą flagi `--generated-at "2026-04-26T00:00:00Z"` w interfejsie wiersza poleceń (CLI). Identyczne kopie zapasowe również wymagają tych samych zabezpieczeń `--anonymization-key-file`, stanu źródła, opcji i wersji kolektora. Dziennik audytu rejestruje `generated_at_pin: ...` za każdym razem, gdy flaga jest ustawiona, dzięki czemu "ustawienie" jest widoczne w analizie kryminalistycznej. Żadna zmienna środowiskowa nie ustawia tej wartości. |
| `engine` | ciąg znaków | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"oracle"`, `"parquet"` lub `"avro"`. `oracle` pojawia się tylko w wynikach podglądu Oracle. |
| `engine_version` | ciąg znaków | Numeryczna wersja produktu bazy danych źródłowej; pusta dla źródeł w postaci plików strukturalnych. Banery dystrybucyjne są wykluczone. |
| `source_kind` | ciąg znaków | Źródła baz danych wykorzystują deklarowane przez operatora wartości `"production"`, `"staging"`, `"scrubbed-replica"` lub `"synthetic"`. Źródła ustrukturyzowane wykorzystują `"parquet"` lub `"avro"`. |
| `length_metadata` | ciąg znaków | Podsumowanie: znacznik zachowany dla wcześniejszych czytelników: `"hybrid-v2"`, `"exact"`, `"rounded"` lub `"not-captured"`. Trzy poniższe pola są wiążące. |
| `declared_length_fidelity` | string | `"exact"` dla zadeklarowanych pojemności znakowych PostgreSQL oraz domyślnego zrównoważonego i dokładnego trybu MySQL; `"coarse-rounded-v1"` dla ścisłej prywatności MySQL; `"not-captured"`, gdy niedostępne. |
| `index_length_fidelity` | string | `"exact"` dla domyślnych zrównoważonych/dokładnych prefiksów indeksów MySQL; `"rounded-down-v1"` dla ścisłej prywatności; `"not-captured"`, gdy niedostępne. |
| `observed_length_fidelity` | string | Domyślnie `"relative-rounded-v2"`, gdy wykonano próbkowanie, `"exact"` w trybie dokładnym, `"coarse-rounded-v1"` w trybie ścisłym albo `"not-sampled"`. Pokrycie próbkowaniem pozostaje oddzielnym wymaganiem dla każdej kolumny. |
| `[totals]` | inline table | Zagregowane liczby (zobacz niżej). |
| `[network]` | table | Opcjonalny dowód połączenia klient-baza i RTT zapytań. |
| `[database_topology]` | tabela | Wymagane dla źródeł baz danych korzystających ze schematu v6 i nowszych. Schemat v7 wykorzystuje kontrakt topologii v2 i rejestruje zakres liczby każdego elementu. Nie występuje w przypadku plików strukturalnych. |
| `[dataset_scope]` | tabela | Wymagane dla każdego Blueprint w wersji schema-v6 i nowszych. Określa, co obejmują sumy i czy pokrycie tabel, wierszy i bajtów jest kompletne. |
| `[structure_scope]` | tabela | Wymagane w wersji 7. Oddzielnie określa stopień kompletności danych dotyczących tabel, kolumn, indeksów i relacji. |
| `[source_environment]` | tabela | Wymagane w bazach danych w wersji 7, a zabronione dla plików strukturalnych. Zawiera jedynie ogólne capacity/hosting dane, które zostały zaobserwowane poprzez interfejs bazy danych lub dedykowanego dostawcę. |
| `[statistics_evidence]` | tabela | Wymagane w wersji 7. Dokładna suma klasyfikacji dotyczących liczby wierszy w każdej tabeli, statystyk optymalizatora oraz danych dotyczących rozmiaru. |
| `[activity_snapshot]` | tabela | Nie napisane przez DBWarp Blueprint 1.6. |
| `[tables.X]` | tables | Jedna dla każdej tabeli, zanonimizowany identyfikator. |
| `[fk_edges]` | inline table | Graf kluczy obcych między zanonimizowanymi tabelami. Opcjonalny. |
| `[artifact_inventory]` | tabela | W wersji 7 wymagane jest rozróżnienie między wartościami "nie żądane", "nie dotyczy", "nieczytelne" oraz zweryfikowanym wynikiem zerowym dla liczby obiektów. Zawiera ograniczone, niezależne od nazw, liczby obiektów, opcjonalne, typowane, anonimowe relacje, wymagania oraz ograniczony spis języków. |

## `[totals]`

| Pole | Typ | Dokładność |
|---|---|---|
| `table_count` | int | exact |
| `row_count` | int | suma zserializowanych danych dla każdej tabeli `rows`; szacunki katalogu są zaokrąglone, natomiast w pełni zweryfikowane i kompletne odczyty są dokładne. |
| `table_bytes` | int | suma zaokrąglonych wartości `table_bytes` poszczególnych tabel |
| `index_bytes` | int | suma zaokrąglonych wartości `index_bytes` poszczególnych tabel |

Te liczby nie są automatycznie sumami dla całego klastra. Należy je zawsze interpretować w połączeniu z `[dataset_scope]`. Shardowany gateway lub koordynator może udostępniać katalog, który wygląda na kompletny, jednocześnie nie zawierając żadnych z podstawowych shardów; schematy v6 i v7 wyraźnie odzwierciedlają tę niepewność, zamiast cicho traktować lokalne statystyki katalogu jako globalną prawdę.

`row_count` to suma arytmetyczna zserializowanych wartości dla każdej tabeli, a nie druga, niezaokrąglona wartość. Znana, dodatnia liczba poniżej pierwszego progu prywatności jest reprezentowana jako `100` z wartościami dla każdej tabeli `row_count_quality = "engine-estimate"`; baza danych zawierająca wiele takich małych tabel może zatem mieć konserwatywnie wysoką sumę. W takim przypadku `dataset_scope.limitations` również zawiera `row-counts-statistical`. Zero pozostaje zarezerwowane do oznaczania, że tabela źródłowa jest pusta.

## `[database_topology]` (źródła baz danych)

Ten blok zapisuje wyłącznie ograniczone fakty widoczne przez połączony endpoint
bazy danych. Nigdy nie zapisuje nazw węzłów lub hostów, adresów IP, nazw
klastrów lub kanałów replikacji, identyfikatorów serwerów ani endpointów.

| Pole | Wartości / reguła |
|---|---|
| `contract` | `dbwarp-blueprint-topology/v1` w schemacie v6; `dbwarp-blueprint-topology/v2` w v7. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed` albo `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, `physical-standby`, `logical-standby`, `snapshot-standby` lub `unknown`. |
| `visibility` | `full`, `partial` albo `unknown`; opisuje dowód topologii, nie poprawność danych. |
| `member_count` | Liczba członków widocznych przez udane zapytania dowodowe. `0` oznacza nieznaną liczbę, nigdy brak członków. |
| `member_count_scope` | Tylko w wersji V7: `deployment`, `visible-subset`, `connected-member` lub `unknown`. Pełna widoczność topologii wymaga `deployment`; `connected-member` wymaga wartości równej jeden. |
| `identifiers_redacted` | Musi mieć wartość `true`. |
| `role_counts` | Opcjonalne liczniki według zamkniętego tokenu roli. Pełna widoczność wymaga, aby ich suma równała się `member_count`. |
| `features` | Posortowane, zamknięte tokeny, takie jak `citus`, formularze MySQL replication/cluster, `postgresql-streaming-replication`, `sqlserver-availability-group`, `oracle-non-cdb`, `oracle-cdb`, `oracle-pdb`, `oracle-rac`, `oracle-data-guard` lub `vitess`. |
| `catalogs_read` | Posortowane zamknięte etykiety poprawnie odczytanych katalogów topologii. |
| `catalogs_unreadable` | Posortowane zamknięte etykiety nieczytelnych katalogów topologii. Każdy wpis wyklucza deklarację pełnej widoczności. |
| `catalogs_not_applicable` | Tylko wersja V7. Posortowane, zamknięte etykiety okazały się nieodpowiednie dla tego źródła. Jest odseparowane od zbiorów czytelnych i nieczytelnych. |

Zwykły punkt końcowy może prawidłowo zgłaszać `deployment = "unknown"`, a jednocześnie zgłaszać kompletne lokalne statystyki tabel z pełnej kopii. Blueprint nie uznaje zwykłego serwera za jednowęzłowy tylko dlatego, że nie było widać żadnej funkcji klastra.

## `[dataset_scope]` (schemat wersji 6 i nowszych)

Ten blok kwalifikuje każdą sumę całkowitą niezależnie. Nie traktuj tych sum jako wartości dla całego zbioru danych, gdy wymagany poziom kompletności wynosi `incomplete` lub `unknown`.

| Pole | Wartości / reguła |
|---|---|
| `contract` | Zawsze `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset` albo `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete` albo `unknown`. |
| `row_count_completeness` | `complete`, `incomplete` albo `unknown`. |
| `size_completeness` | `complete`, `incomplete` albo `unknown`. |
| `row_count_method` | Zamknięty token pochodzenia, taki jak `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter`, `oracle-table-statistics` lub `oracle-segment-statistics`. `bounded-complete-read` i `mixed-catalog-and-bounded-read` identyfikują sumy odzyskane z w pełni zweryfikowanego odczytu poziomu 2, samodzielnie lub wraz z liczbami katalogów. Oracle używa `not-applicable` wraz z tą samą metodą rozmiaru tylko wtedy, gdy inwentarz nie jest pusty, a w populacji sum ogółowych nie ma tabel. `distributed-aggregate` jest akceptowany jako dane wejściowe, ale nie jest zapisywany przez tę wersję. |
| `size_method` | Zamknięty token pochodzenia, taki jak `postgres-local-relation-size`, `citus-distributed-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `oracle-segment-bytes`, `oracle-table-logical-estimate`, `mixed` lub `not-applicable`. Oracle używa `mixed`, gdy dołączone tabele łączą zliczniki segmentów atrybutów z oznaczonymi logicznymi oszacowaniami. Używa `not-applicable` tylko wtedy, gdy inwentarz nie jest pusty, a jednocześnie nie ma żadnych tabel w populacji sumy kopii, aby całkowity wynik zerowy nie fałszywie sugerował metodę pomiaru. `distributed-aggregate` jest akceptowany jako dane wejściowe, ale nie jest zapisywany przez tę wersję. |
| `limitations` | Posortowane zamknięte przyczyny niepełnego lub nieznanego pokrycia. Co najmniej jedna jest wymagana, chyba że wszystkie wymiary są kompletne. |

`selection-limited` oznacza, że sumy i deklaracje kompletności obejmują dokładnie schematy wskazane powtarzalnym selektorem trybu na żywo `--schema`; nie deklarują pokrycia całej połączonej bazy danych. Pominięcie `--schema` zachowuje przechwytywanie wszystkich widocznych schematów.

Czytelna, wybrana schemat może zawierać wyłącznie obiekty inne niż tabele i jest przechowywana w odpowiednich rejestrach. Jednak, gdy pełne zebranie nie zawiera żadnych tabel, kolektor nie powinien publikować ostatecznego, pustego zbioru danych: kompletność danych tabel, wierszy i rozmiarów pozostaje niekompletna, a `table-inventory-visibility-unknown` rejestruje ostrożne ograniczenie.

`row-count-evidence-incomplete` i `size-evidence-incomplete` oznaczają, że co najmniej jedna z uwzględnionych tabel nie posiadała odpowiadającej jej wartości w katalogu. Suma numeryczna to suma znanych wartości, a nie stwierdzenie, że niedostępna tabela zawierała zero wierszy lub bajtów. Statystyki dla poszczególnych tabel wskazują, które rekordy są niedostępne.

Dla Oracle, `oracle-segment-bytes` jest preferowanym, dokładnym dowodem na alokowany rozmiar. Jeśli nie można przypisać miejsca do tabeli na podstawie `DBA_SEGMENTS` – na przykład dla tabeli klastrowanej lub tabeli zorganizowanej indeksowo, której katalog indeksów jest niedostępny – kolektor może wygenerować `oracle-table-logical-estimate` używając już dostępnych wartości `DBA_TABLES.NUM_ROWS * AVG_ROW_LEN`. Dowód dla tabeli zawiera wtedy `size_quality = "engine-estimate"`, `size_scope = "table-only"` oraz `size_accounting = "logical-estimate"`, `size_visibility = "partial"`, a kompletność rozmiaru zbioru danych wynosi `incomplete`; nie deklaruje rozmiaru LOB ani indeksów. Brakujący katalog doprecyzowań nigdy nie powoduje, aby już przypisane do tej logicznej tabeli bajty zostały usunięte: zmierzony wkład pozostaje `oracle-segment-bytes`, z ograniczoną widocznością i niepełnym pokryciem zagregowanych danych. Nieprzypisywalna, współdzielona lub zorganizowana indeksowo przestrzeń nie jest publikowana jako dokładna wartość zero. Tabela Oracle z indeksem domenowym również korzysta z ograniczonej widoczności i nieznanego zakresu, ponieważ implementacje domenowe, takie jak Text i Spatial, mogą przechowywać bajty w obiektach pomocniczych poza wygenerowanym katalogiem tabel użytkownika. Logiczne rozwiązanie awaryjne to dowód na rozmiar kopii, a nie licznik alokowanych bajtów. Może zawyżać aktualną alokację, gdy liczby wierszy optymalizatora pozostają nieaktualne po zwolnieniu miejsca (na przykład po `TRUNCATE ... DROP STORAGE`), a pochodzenie tej estymacji musi być zachowane. Nie jest wymagane żadne uprawnienie `DBA_TABLESPACES` dla tego rozwiązania awaryjnego.

Dla indeksów w bazie danych Oracle, zakończenie odczytu katalogu indeksów stanowi granicę logicznego stanu. Wiersz `DBA_SEGMENTS` występujący później, który nie ma odpowiadającego mu identyfikatora indeksu, znajduje się poza tym zestawem danych i nie jest przypisywany do żadnej tabeli. Indeks, który istniał w momencie tej granicy, ale nie zawiera oczekiwanego fragmentu danych, nie zapewnia pełnej widoczności dla swojej tabeli.

`logical-partition-root-unmeasured` to specyficzny dla PostgreSQL dowód, że dołączony logiczny element główny celowo nie dodaje ani wierszy, ani bajtów, ponieważ te wartości znajdują się na jego fizycznych podstrukturach. W przeciwieństwie do `row-count-evidence-incomplete`, który można naprawić, w przypadku braku statystyk, pełne odczytanie innej tabeli nie przywróci kompletności danych na poziomie zbioru, dopóki taki element główny pozostaje w wybranym zestawie.

`table-inventory-visibility-unknown` oznacza, że kolektor nie mógł odczytać klasyfikacji należącej do silnika, która jest potrzebna do rozróżnienia obiektów użytkownika od obiektów pomocniczych. Widoczne rekordy mogą nadal być obecne, ale kompletność tabeli, wierszy i rozmiaru jest nieważna, zamiast traktować ten podzbiór jako całość.

`catalog-capture-truncated` oznacza, że sesja katalogu, korzystająca z jednego źródła, została zakończona przed odczytaniem wszystkich planowanych grup lub wybranych schematów. Zapisy, które zostały już uznane za kompletne, mogą zostać przesłane, ale twierdzenie o kompletności zbioru danych lub struktury nie może obejmować niezczytanej części.

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

## `[structure_scope]` (schemat v7)

Ten blok pozwala odróżnić zweryfikowany, pusty katalog od katalogu, który został przefiltrowany, jest nieczytelny lub nie został sprawdzony.

| Pole. | Wartości / reguła. |
|---|---|
| `contract` | Zawsze `dbwarp-blueprint-structure-scope/v1`. |
| `visibility` | `full`, `privilege-filtered` lub `unknown`. Kompletność danych obejmuje wybrane schematy i zakres uprawnień; nie jest to gwarancja nieograniczonego dostępu do bazy danych. |
| `table_inventory_completeness`, `column_inventory_completeness`, `index_inventory_completeness`, `relationship_inventory_completeness` | Niezależnie od `complete`, `incomplete` lub `unknown`. Zależne rodziny nie mogą oznaczać jako kompletne, jeśli wymagana rodzina nadrzędna jest niekompletna. |
| `catalogs_read`, `catalogs_unreadable`, `catalogs_not_applicable` | Posortowane, rozłączne, zamknięte etykiety katalogów. `catalogs_read` oznacza potwierdzenie poprawnego odczytu; w przypadku przechwytywania danych wielu właścicieli może zachować katalog, jeśli odczyt co najmniej jednego oczekiwanego właściciela zakończył się pomyślnie, nawet jeśli dla innego właściciela się nie powiódł. `catalogs_unreadable` oznacza, że nie odnotowano żadnego pomyślnego odczytu. Kompletny zestaw wymaga specyficznego dla silnika katalogu w `catalogs_read`, aby każde zapytanie do tego zestawu zakończyło się pomyślnie, oraz aby nie występowały żadne braki danych dla poszczególnych obiektów. |
| `limitations` | Posortowane kody przyczyn, takie jak `selection-limited`, `metadata-visibility-privilege-filtered` lub `table-kinds-not-inventoried`. Częściowe lub nieznane dowody wymagają podania przyczyny. |

Selektory schematów są częścią zakresu: `complete` oznacza pełny zakres dla wybranych, rozwiązanych schematów, ale niekoniecznie wszystkie schematy w systemie. Selektor, który nie rozwiązuje się do żadnego schematu, jest błędem i nie może prowadzić do utworzenia kompletnego, pustego Blueprintu.

`catalog-capture-truncated` ma to samo znaczenie w kontekście weryfikacji struktury: opublikowane rekordy tabel i kolumn stanowią zweryfikowany prefiks lub podzbiór właściciela, a nie twierdzenie, że pozostała część planowanych operacji na katalogu została zakończona. Katalog, który nie został przetworzony przez daną instancję, nie pojawia się w żadnym z trzech zestawów katalogów; nie należy go ponownie oznaczać jako nieczytelny lub nieistotny.

Dla operacji odczytu z wieloma właścicielami, `index-inventory-unavailable` lub `relationship-inventory-unavailable` mogą towarzyszyć katalogowi w `catalogs_read`: etykieta katalogu zachowuje pozytywne potwierdzenie właściciela, który zakończył operację pomyślnie, podczas gdy pole kompletności i rekord ograniczeń wskazują, że nie zaobserwowano całej wybranej populacji. Ograniczenia na poziomie tabeli identyfikują obiekty, dla których występuje luka w reprezentatywności; nie zastępują one dowodów na status zapytania dla właściciela, któremu odmówiono dostępu lub który nie próbował wykonać operacji i nie wygenerował żadnych tabel.

W systemie Oracle rekordy `oracle-identity-columns` i `oracle-constraint-columns` są przechowywane oddzielnie od katalogów kolumn i ograniczeń nadrzędnych. Ich obecność lub brak opisuje opcjonalne mechanizmy generowania identyfikatorów i dowody na istnienie kluczy relacyjnych; nie można tego zinterpretować jako stwierdzenie, że katalog nadrzędny był niedostępny.

## `[source_environment]` (źródła baz danych ze schematu v7)

Ten blok nigdy nie opisuje stacji roboczej, na której działa `dbwarp-blueprint`. `collector_machine_excluded` musi być `true`. Dowody dotyczące wydajności pochodzą wyłącznie z podłączonego punktu końcowego bazy danych lub od wyraźnie autoryzowanego dostawcy, koordynatora lub operatora.

| Pole. | Wartości / reguła. |
|---|---|
| `contract` | Zawsze `dbwarp-blueprint-source-environment/v1`. |
| `evidence_origin` | `database-endpoint`, `provider-api`, `orchestrator-api`, `operator-attested`, `mixed` lub `none`. |
| `hosting_model` | `managed-service`, `self-managed`, `orchestrated` lub `unknown`. |
| `infrastructure_location` | `cloud`, `on-premises`, `hybrid` lub `unknown`. |
| `capacity_scope` | `connected-instance`, `database-resource`, `cluster-aggregate`, `member-subset` lub `unknown`. |
| `capacity_visibility` | `capacity_visibility` może być `full`, `partial`, `unknown` lub `not-requested`. `not-requested` wymaga nieznanych zakresów, podstaw i obszaru działania, a także nie posiada sklasyfikowanego katalogu pojemności. Klasyfikacja niezwiązana z pojemnością, taka jak edycja SQL Server, może być nadal obecna. |
| `cpu_capacity_band` | `1`, `2`, `3-4`, `5-8`, `9-16`, `17-32`, `33-64`, `65-128`, `129-plus` lub `unknown`. |
| `cpu_capacity_basis` | `logical-cpu-limit`, `database-resource-limit`, `operating-system-visible`, `physical-host` lub `unknown`. `operating-system-visible` nie twierdzi, że przydzielenie maszyny wirtualnej, kontenera lub usługi zarządzanej odpowiada fizycznemu hostowi. |
| `memory_capacity_band` | Grube zakresy od `under-2-gib` do `512-gib-plus`, lub `unknown`. |
| `memory_capacity_basis` | `database-buffer-cache`, `database-resource-limit`, `operating-system-visible`, `physical-host` lub `unknown`. `operating-system-visible` stanowi podstawę dla silnika DMV, którego wartość może opisywać środowisko wirtualne lub kontener, a nie fizyczny serwer. `database-buffer-cache` to alokacja pamięci podręcznej, a zatem tylko dolna granica całkowitej ilości pamięci źródła; nigdy nie należy jej przedstawiać jako pojemność hosta bez uwzględnienia tej podstawy. |
| `member_capacity_uniform` | Opcjonalne observed/attested – wartość logiczna; pominięcie oznacza "nieznane". |
| `features` | Posortowane, zamknięte wpisy, takie jak `autoscaling`, `burstable`, `container-limits-visible`, `database-resource-governed`, `serverless` lub `shared-host`. |
| `limitations` | Posortowane, zamknięte ograniczenia pochodzenia. `oracle-client-version-mismatch` lub `oracle-client-version-unreadable` wskazują, że wersja klienta Oracle SQL*Plus nie mogła zostać w pełni potwierdzona. `oracle-client-version-below-tested-floor` wskazuje potwierdzonego klienta starszego niż 12.1, czyli próg porównawczy zapisany w tym kontrakcie. Pobieranie katalogu jest kontynuowane, ponieważ pochodzenie banera klienta nie określa struktury bazy danych. |
| zbiory katalogów | Posortowane, odrębne dowody dotyczące dokładnych katalogów środowiska źródłowego, w tym klasyfikacja edycji SQL Server, nawet jeśli jej opcjonalna funkcja DMV (Dynamic Management View) jest niedostępna. |

Nieznana pojemność nie oznacza pojemności zerowej. Połączenie zdalne nie upoważnia do odczytywania zużycia procesora lub pamięci hosta, na którym działa program, i przekształcania tych danych w informacje o pojemności serwera.

Dla systemu Oracle, katalog pojemności, który został ukończony tylko dla części planowanego zestawu zapytań, pozostaje pozytywnym `catalogs_read` dowodem, ale jego wartości są ukrywane, a `capacity_visibility` jest `unknown`. Częściowe wiersze nie powinny być przedstawiane jako globalne limity procesora lub pamięci.

Ustawienia funkcji Oracle SQL*Plus są wybierane z numerycznej wersji działającej sesji, jeśli jest czytelna, w przeciwnym razie z banera pliku wykonywalnego, a na końcu z konserwatywnego protokołu, który nie wybiera ani `ROWLIMIT`, ani znaczników CSV jako funkcji wyjściowych. Oba przebiegi ustawień nadal próbują wyczyścić odziedziczone `ROWLIMIT` i tryb CSV; diagnostyka nieznanej opcji starszego klienta jest tolerowana tylko w wyznaczonym oknie resetowania. Niezgodność wersji, częściowe potwierdzenie, błąd parsowania lub potwierdzony klient starszy niż 12.1 osłabiają jedynie pochodzenie danych. Nigdy nie blokują pobierania katalogu.

## `[statistics_evidence]` i `[tables.<id>.statistics]` (schemat wersji 7)

Każda tabela w wersji v7 posiada blok statystyk. Blok najwyższego poziomu zawiera dokładne liczby dla `statistics_state`, `row_count_quality` i `size_quality`; każdy zestaw musi obejmować wszystkie tabele i dokładnie odpowiadać klasyfikacjom na poziomie tabeli. Agregowana widoczność wynosi `full` tylko dla niepustej populacji kopii, gdy każda zliczana tabela ma pełną widoczność rozmiaru, znane dane dotyczące wierszy i sklasyfikowany stan statystyk, oraz żaden katalog statystyk nie jest nieczytelny. Celowo wykluczone obiekty zewnętrzne, tymczasowe i pochodne pozostają w inwentarzu; ich brak dostępności wierszy i rozmiaru, wynikający z zasad, nie obniża widoczności tej populacji kopii, ale nieklasyfikowany stan statystyk nadal to robi. Gdy wszystkie tabele są wykluczone, agregowana widoczność wynosi `unknown` z `statistics-visibility-unknown`; pusta populacja nie powinna otrzymywać `full` w sposób bezpodstawny. `catalog-capture-truncated` rejestruje, że planowane prace nad katalogiem statystyk zostały przerwane, zanim dotknięto wszystkich właścicieli, przy zachowaniu wszelkich pozytywnych dowodów odczytu katalogu, które zostały już uzyskane. Ta sama zasada pozytywnych dowodów obowiązuje, gdy jeden właściciel odczytuje pomyślnie, a drugi jest odrzucany: katalog pozostaje w stanie `catalogs_read`, podczas gdy `statistics-partial` i agregowana widoczność rejestrują, że wybrana populacja nie została w pełni zaobserwowana.

Pola na poziomie tabeli to:

| Pole. | Wartości / reguła. |
|---|---|
| `row_count_method` | Engine/version-aware metoda katalogu, `bounded-complete-read` gdy instrukcja poziomu 2 bezpiecznie wypisała widoczną tabelę, licznik plików strukturalnych lub `unknown`; zwykłe pobieranie nie przechodzi automatycznie do `COUNT(*)`. |
| `row_count_quality` | `exact-counter`, `exact-read`, `engine-counter`, `engine-estimate`, `cached-engine-estimate`, `sample-extrapolation`, `unavailable` lub `unknown`. Znany, pozytywny licznik SQL Server, znajdujący się poniżej pierwszego niezerowego zakresu prywatności, używa `engine-estimate` po serializacji `rows = 100`; to odróżnia zakres prywatności zarówno od dokładnego licznika, jak i zmierzonej wartości zerowej. |
| `statistics_state` | `current`, `possibly-stale`, `known-stale`, `never-analyzed`, `locked`, `user-supplied`, `not-applicable` lub `unknown`. |
| `refresh_age_band` | `under-1h`, `1h-1d`, `1-7d`, `1-4w`, `1-3m`, `3m-plus`, `unknown` lub `not-applicable`. |
| `modification_ratio_band` | `none`, `under-1pct`, `1-5pct`, `5-10pct`, `10-20pct`, `20-50pct`, `over-50pct`, `unknown` lub `not-applicable`. |
| `sample_fraction_band` | `full`, `75-99pct`, `50-74pct`, `25-49pct`, `under-25pct`, `unknown` lub `not-applicable`. |
| `statistics_scope` | `global`, `partition`, `subpartition`, `session`, `local-member`, `logical-dataset`, `database-resource`, `structured-dataset`, `selected-object` lub `unknown`. |
| `size_method`, `size_quality`, `size_scope`, `size_accounting`, `size_visibility` | Oddzielnie opisz, skąd pochodzi informacja o rozmiarze, czy jest to wartość licznikowa, czy oszacowanie, czy uwzględnia LOB/index przestrzeń dyskową, czy jest to rozmiar przydzielony, czy logiczny, oraz czy dostępność jest pełna, częściowa, niedostępna, czy nieznana. |

Blok statystyk najwyższego poziomu używa `visibility = "full"`, `"partial"` lub `"unknown"` do określenia całkowitej liczby kopii, o której mowa powyżej. Nigdy nie używa tylko obiektów wykluczonych, aby uzyskać widoczność `full`.

Dowody dotyczące Oracle `oracle-segment-bytes` są ważne tylko w połączeniu z `exact-counter`, `allocated-segment`, pełną lub częściową widocznością oraz `segment_state`, które potwierdzają, że dane dotyczące segmentu zostały przypisane (`created`, `deferred`, `mixed` lub `mixed-table-and-index`). Alternatywnie, w przypadku wystąpienia problemów, można skorzystać z `oracle-table-logical-estimate`, `engine-estimate`, `logical-estimate`, częściowej widoczności oraz stanu segmentu, który jest niedostępny. To zapobiega temu, aby nieokreślona klasa pamięci stała się wartością zerową w pomiarach. Stan jest wyznaczany na podstawie danych katalogowych, a następnie poddawany obróbce w celu ochrony prywatności. `created` może więc towarzyszyć serializowanym zerowym rozmiarowi tabeli, gdy znana, pozytywna wartość licznika surowych tabel spada poniżej pierwszego zakresu bajtów. Częściowe, zmierzone dane dotyczące Oracle, używające `size_scope = "unknown"`: Przypisane bajty pozostają dokładne, ale brakujący obiekt LOB, struktura zagnieżdżonego przechowywania lub mapowanie indeksu oznacza, że narzędzie do zbierania danych nie może uczciwie twierdzić, że obejmuje pełny zakres table/LOB/index. `mixed` oznacza, że przydział indeksu o określonej wartości został pominięty i zastąpiony wartością `index_bytes = 0` w wyniku zaokrąglenia; w ten sposób zero jest odróżnione od przypadku tabeli, dla której analiza segmentu nie wykazała żadnego przydziału indeksu. `mixed-table-and-index` oznacza, że zarówno alokacje dla tabeli, jak i indeksu były dodatnie przed zaokrągleniem, a oba zserializowane liczniki mają wartość zero, zachowując oba te fakty bez ujawniania wartości bajtów dla poszczególnych podzbiorów. `deferred` oznacza, że przypisane surowe wartości liczników były równe zero i wymaga, aby oba zserializowane wartości bajtowe również były równe zero. Użyj informacji o stanie, aby odróżnić zaokrąglone przydzielenie pod-segmentu od przestrzeni dyskowej, co do której dowiedziono, że nie została zmaterializowana.

Blok najwyższego poziomu również rejestruje posortowane, rozłączne katalogi oraz ograniczenia. Wartość zero `rows` lub `table_bytes` może być używana jako zaobserwowana wartość zero tylko w połączeniu z odpowiednimi dowodami quality/visibility; nie ignoruj bloku pochodzenia. Zliczona tabela, której jakość wierszy lub rozmiaru jest `unavailable` lub `unknown`, powoduje, że kompletność zestawu danych odbiega od wartości `complete`; walidator odrzuca wartość numeryczną przedstawioną jako pełne pokrycie. W szczególności, tabela PostgreSQL, która nie ma statystyk optymalizatora ani udowodnionej kompletnej, ograniczonej możliwości odczytu, ma nieznaną liczbę wierszy, a nie zmierzoną wartość zero.

Wersja Oracle Basic pomija `check_count`, gdy słownik nie jest w stanie odróżnić zadeklarowanego `NOT NULL` ograniczenia od jawnego, tekstowo identycznego `CHECK`. Nie odgaduje na podstawie nazwy wygenerowanego ograniczenia ani aktualnej wartości nullability kolumny. Inne tabele, których wiersze z ograniczeniami są jednoznaczne, mogą nadal zawierać dokładną liczbę.

## `[activity_snapshot]`

DBWarp Blueprint 1.6 nie zapisuje tego bloku.

## `[network]` (opcjonalne)

Czas odpowiedzi (round-trip time) od maszyny, na której działa narzędzie do zbierania danych, do Twojej bazy danych. Nie jest to czas odpowiedzi między źródłem a celem migracji.

Sonda działa po ustanowieniu połączenia, a przed zapytaniami katalogowymi, więc
rozgrzewanie pamięci podręcznej zapytań nie zniekształca pomiarów. Wykonuje
**5× `SELECT 1`** i emituje medianę opóźnienia. Każde `SELECT 1` zwraca stałą
liczbę całkowitą 1 — sonda nigdy nie odczytuje danych wierszy.

Brakujący, gdy używany jest `--no-rtt-probe` lub gdy samo badanie nie powiodło się w trakcie działania (zarejestrowane jako ostrzeżenie niekrytyczne do stderr i dziennika audytu; plik Blueprint jest nadal generowany, pomijając ten blok).

| Pole | Typ | Dokładność |
|---|---|---|
| `sample_count` | int | exact (zawsze 5 w v1) |
| `connect_total_ms` | int | Całkowity czas zegarowy od rozpoczęcia połączenia TCP do gotowej uwierzytelnionej sesji, w milisekundach. Obejmuje uzgadnianie TCP + uzgadnianie TLS (gdy ma zastosowanie) + wyzwanie/odpowiedź uwierzytelniania. Zaokrąglany do najbliższej ms. Zwykle 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | Mediana opóźnienia pojedynczego obiegu dla 5 próbek `SELECT 1`, w milisekundach. Zaokrąglana do najbliższej ms. Naturalny poziom szumu sieciowego (w praktyce ≥ 1 ms) jest szerszy niż dokładność zaokrąglenia, co usuwa ukryty kanał w najmniej znaczących bitach bez utraty użytecznej dokładności. Wartości LAN poniżej ms zapadają się do 0 lub 1. |
| `query_rtt_ms_p95` | int | 95. percentyl dla 5 próbek, obliczony metodą najbliższej rangi (najwolniejsza obserwacja), w milisekundach. Zaokrąglany do najbliższej ms. Wraz z p50 pomaga wykryć krótkie skoki opóźnienia; pięć próbek służy jedynie do orientacji i nie stanowi testu wydajności obciążenia roboczego. |

Pięć zapytań sondujących pojawia się w dzienniku audytu jako **jeden wpis podsumowujący** (a nie pięć oddzielnych wierszy) z etykietą `5x SELECT 1 (RTT probe; constant integer 1, no row data)`. Jest to zgodne z założeniem zaufania, że żadna zawartość wierszy nie jest odczytywana.

## `[tables.<id>]`

Identyfikator to `table-NNN`, gdzie `NNN` to indeks liczony od 1 w kolejności HMAC-SHA256, oddzielonej domenami, nazw schematu i tabeli. Domyślny klucz jest generowany na nowo dla każdego procesu i nigdy nie jest ujawniany. Przekazanie tego samego, chronionego `--anonymization-key-file` zachowuje kolejność podczas zatwierdzonych porównań. Schemat w wersji 7 wymaga kompletnego, ciągłego zestawu liczb od `table-001` do liczby tabel (szerokość naturalnie rośnie w `table-1000`); pomijane, zerowe, nie dziesiętne lub pochodzące ze źródła sufiksy są nieprawidłowe.

| Pole | Typ | Dokładność / wartości |
|---|---|---|
| `rows` | int | Szacunki dotyczące katalogów są zaokrąglane: do najbliższej 100 (≤10 tys.), 1000 (≤1 mln), 10000 (>1 mln). Znany, dodatni szacunek, który w innym przypadku zostałby zaokrąglony do zera, używa pierwszego niezerowego przedziału (`100`); zero jest zarezerwowane dla katalogu o zerowej wartości lub niedostępnych danych, co jest sygnalizowane przez sąsiednie wskaźniki jakości. Gdy ograniczony odczyt poziomu 2 potwierdzi, że zliczył całą widoczną tabelę, `rows` to dokładna liczba, która została już ujawniona przez dokładną wartość `sample_rows` tego próbkowania; zapobiega to sprzecznościom w liczbie rekordów, kardynalności i sumach dla poszczególnych tabel, bez dodawania nowego kanału. |
| `table_bytes` | int | rounded: najbliższe 1KiB / 1MiB / 100MiB zależnie od wielkości |
| `index_bytes` | int | rounded: tak samo jak `table_bytes` |
| `schema` | ciąg znaków | Zanonymizowane identyfikatory: `schema-A`, `schema-B`, ..., `schema-AA`. Schemat v7 wymaga kompletnego, alfabetycznego zestawu wartości dla każdego schematu odniesionego przez tabelę lub artefakt graph/analyzed; dlatego zachowywany jest wybrany schemat zawierający tylko obiekty inne niż tabele. |
| `object_kind` | ciąg znaków | Wersja V7 wymagała zamkniętego tokenu: `ordinary-table`, `materialized-view`, `external-table`, `temporary-table`, `nested-table` lub `object-table`. Tożsamość obiektu jest niezależna od fizycznego przechowywania i partycjonowania. |
| `storage_organization` | ciąg znaków | Wersja V7 wymaga zamkniętego tokenu: `heap`, `index-organized`, `clustered`, `external` lub `unknown`. `external` jest prawidłowe tylko dla `object_kind = "external-table"`. |
| `partitioning` | ciąg znaków | Wersja V7 wymagała zamkniętych tokenów: `none`, `range`, `list`, `hash`, `interval`, `reference`, `composite`, `system`, `key`, `linear-hash`, `linear-key` lub `unknown`. |
| `segment_state` | ciąg znaków | Wersja V7 wymagała zamkniętego tokenu: `created`, `deferred`, `mixed`, `mixed-table-and-index`, `unavailable` lub `unknown`. To rozróżnia obiekty zawierające tylko metadane od obiektów z materializowanym przechowywaniem. Jest to kategoryczny dowód, ustalony przed zaokrągleniem bajtów, więc `created` może występować wraz z zerową liczbą zserializowanych bajtów tabeli, wskazując na pozytywne przydzielenie pod-segmentu. W przypadku dowodu dotyczącego liczników segmentów w Oracle, `mixed` rejestruje pozytywne przydzielenie indeksu, którego zserializowana wartość `index_bytes` zaokrągła się do zera; `mixed-table-and-index` rejestruje, że oba przydziały (surowe i zserializowane) były pozytywne, podczas gdy oba liczniki zaokrągły się do zera. |
| `parent_table`, `child_tables` | ciąg znaków / tablica. | Opcjonalne, wzajemne, anonimowe połączenia tabel dla obiektów zagnieżdżonych, partycjonowanych lub w inny sposób zawartych. Identyfikatory potomnych są posortowane i unikalne; graf rodzicielski musi być acykliczny. |
| `table_features` | tablica | Posortowane, zamknięte tokeny: `graph-edge`, `graph-node`, `memory-optimized`, `temporal-current` lub `temporal-history`. |
| `unlogged` | bool | Opcjonalna obserwacja stanu zapisu w PostgreSQL. Pomijana, gdy nie została zarejestrowana; wyraźne `false` oznacza, że katalog potwierdził, że tabela jest zapisywana. |
| `partition_count` | int | Dokładna liczba fizycznych, należących do zakresu, partycji liściowych, wymagana, gdy `partitioning` określa znaną strategię partycjonowania. PostgreSQL raportuje rekurencyjne partycje liściowe i wyklucza partycje spoza zdefiniowanych schematów zgodnie z `selection-limited`. W MySQL, tabele złożone liczą podpartycje, ponieważ są to ich fizyczne partycje liściowe; na przykład, cztery partycje najwyższego poziomu z ośmioma podpartycjami każda raportują `32`. Zero jest prawidłowe tylko dla logicznego korzenia partycji z `segment_state = "unavailable"` i bez partycji liściowych należących do zakresu. |
| `partition_key_cols` | tablica liczb całkowitych | Uzupełnij proste numery kolumn klucza partycji. Pomijane, jeśli klucz jest w całości lub częściowo oparty na wyrażeniach, lub gdy dane katalogu są niedostępne; częściowa lista numerów i wyrażenia klucza nigdy nie są serializowane. |
| `partition_rows_max` | int | Opcjonalne, zaokrąglone oszacowanie liczby wierszy w największej liściastej sekcji. Dla oszacowań sumarycznych dla tabel, znana, dodatnia wartość wykorzystuje pierwszy niezerowy przedział wierszy, ograniczony przez wartość `rows`. W przypadku dokładnego odczytu zawartości tabeli, największe oszaczenie liczby wierszy, którego prywatny przedział byłby zerowy lub przekraczałby dokładną liczbę wierszy, jest pomijane jako niemożliwe do przedstawienia, zamiast być ograniczane do fałszywej wartości. Jeśli jest obecne, nie może być równe zero, gdy `rows` jest dodatnie lub przekracza `rows`. |
| `temporal_history` | ciąg znaków | Anonimowy identyfikator tabeli powiązanej z tabelą historii czasowej, wymagany wraz z funkcją `temporal-current`, chyba że tabela ta zawiera odpowiedni, przypisany do obiektu token `table_limitations`. Wybór obejmujący całą tabelę nigdy nie zwalnia z konieczności utrzymywania tego połączenia. |
| `table_limitations` | tablica | Posortowane, kompletne dane dotyczące każdego obiektu. `table-classification-unavailable` identyfikuje tabelę, dla której dane dotyczące typu obiektu były niekompletne. `column-inventory-unavailable` identyfikuje tabelę zawierającą jeden lub więcej brakujących lub nieczytelnych rekordów kolumn; `dependent-structure-suppressed` wskazuje, że zarówno struktura indeksu, jak i relacji dla tej tabeli nie mogą być uznane za kompletne, ponieważ brakuje jednej z kolumn. `index-inventory-unavailable` i `relationship-inventory-unavailable` zawężają zakres operacji ograniczonych do modyfikacji, wpływając jedynie na powiązane obiekty, bez usuwania wymaganych kolumn z bazy danych. Wszelki indeks, klucz partycji lub relacja, które odwołują się do nieistniejącej kolumny, są pomijane, zamiast powodować unieważnienie całego procesu zbierania danych. `relationship-target-outside-selected-scope` wskazuje, że co najmniej jeden zadeklarowany klucz obcy w tej tabeli odnosi się do obiektu znajdującego się poza zakresem schematu, który został zdefiniowany; Jest ono ważne tylko w przypadku przechwycenia `selection-limited`. `relationship-target-visibility-unknown` wskazuje, że katalog ujawnił cel relacji typu "foreign key", który nie mógł zostać zidentyfikowany w dostępnych zasobach. kompletność relacji musi być wtedy niepełna. `row-security-filter-active` rejestruje widoczny, aktywny filtr SQL Server. `row-security-visibility-unknown` wskazuje, że nie można było udowodnić pełnej widoczności katalogu zasad bezpieczeństwa SQL Server, dlatego pobieranie próbek na poziomie Tier-2 jest wyłączone, zamiast traktować potencjalnie ograniczony podzbiór jako pełną populację tabeli. `temporal-history-outside-selected-scope` jest ważne tylko dla tabeli bez powiązań, odzwierciedlającej aktualny stan, w `selection-limited` podczas zbierania danych, po tym jak kolektor rozwiązał schemat historii poza zakresem wyboru. `temporal-history-visibility-unknown` wskazuje, że katalog ujawnił identyfikator obiektu historycznego, ale nie dostarczył wystarczających metadanych, aby go zidentyfikować. |
| `counted_in_totals` | bool | Pominięte oznacza zawarte. `external-table`, `materialized-view`, `temporary-table`, lub tabela zawierająca `memory-optimized` wymaga wyraźnego określenia `false`, wyłączając dane zewnętrzne, pochodne, związane z sesją lub obecnie niepomierzone z `table_count`, `row_count`, `table_bytes` i `index_bytes`. Dla każdego obiektu dostępne pozostają dane umożliwiające planowanie odzyskiwania, bez przedstawiania niedostępnych wartości jako sumarycznych wyników. Żadna inna, wyraźnie określona wartość nie jest wartością domyślną. |
| `check_count` | int | Opcjonalna, dokładna liczba ograniczeń strukturalnych typu CHECK. Pominięcie oznacza, że wartość jest nieznana; `0` oznacza, że odpowiedni katalog nie zawierał żadnych takich ograniczeń. |
| `has_clustered_index` | bool | zawsze `false` dla PostgreSQL |
| `[tables.<id>.statistics]` | podtabela | Wymagane informacje o pochodzeniu w wersji 7 dotyczące liczby wierszy, stanu statystyk optymalizatora oraz danych dotyczących rozmiaru. Pole `stats_freshness` w wersji 6 jest akceptowane tylko podczas odczytu starszych plików i nigdy nie jest generowane w wersji 7. |
| `[tables.<id>.cols.<cid>]` | sub-tables | jedna dla każdej kolumny |
| `[tables.<id>.idxs.<iid>]` | sub-tables | jedna dla każdego indeksu |
| `[tables.<id>.compression]` | sub-table | tylko przy Poziomie 2 |

## `[tables.<id>.cols.<cid>]`

Identyfikator to `col-N`, gdzie `N` to naturalna kolejność atrybutów kolumny (indeksowana od 1, zachowująca kolejność na dysku). Jest stały w kolejnych uruchomieniach. W schemacie v7, sufiks dziesiętny musi dokładnie odpowiadać `ordinal`; wartości zerowe, wiodące zera oraz etykiety pochodzące z źródła są nieprawidłowe. Silniki źródłowe mogą zachowywać luki w fizycznej kolejności kolumn po usunięciu kolumny.

| Pole | Typ | Uwagi |
|---|---|---|
| `ordinal` | int | to samo N co w identyfikatorze |
| `type` | string | Znormalizowana rodzina typów, taka jak `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"` lub `"user-defined"`. Rzeczywiste nazwy domen, typów wyliczeniowych, aliasów, typów złożonych i typów zdefiniowanych przez użytkownika nie są emitowane. |
| `nullable` | bool |  |
| `value_source` | string | Opcjonalny zamknięty token schematu v6: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time` albo `rowversion`. Pomijany dla zwykłej wartości lub nieznanych danych. |
| `has_default` | bool | Opcjonalna obserwacja katalogu w schemacie v6. Pominięcie oznacza wartość nieznaną; jawne `false` potwierdza brak wartości domyślnej. |
| `default_kind` | string | Opcjonalna klasyfikacja `constant`, `function` albo `expression` w schemacie v6, poprawna tylko z `has_default = true`. Tekst i literały nigdy nie są serializowane. |
| `default_on_null` | bool | Wersja V7, opcjonalna obserwacja katalogu źródłowego dla Oracle `DEFAULT ON NULL`; ważna tylko wtedy, gdy dostępny jest domyślny parametr. "Pominięto" oznacza, że nie została zaobserwowana. |
| `type_kind` | string | Opcjonalny zamknięty token schematu v6: `enum`, `set`, `domain`, `composite`, `array`, `range` albo `alias`. Pomijany dla typu bazowego lub nieznanych danych. |
| `member_count` | int | Dokładna dodatnia strukturalna liczba elementów w schemacie v6, wymagana tylko dla `enum` i `set`. Ich nazwy nigdy nie są serializowane. |
| `domain_has_check` | bool | Opcjonalna obserwacja CHECK domeny w schemacie v6, poprawna tylko z `type_kind = "domain"`. |
| `hidden`, `invisible`, `masked`, `encrypted`, `sparse` | bool | Opcjonalne obserwacje katalogu. `invisible` różni się od ukrytej kolumny utworzonej przez silnik. Pominięto oznacza nieznane; jawne `false` oznacza, że katalog potwierdził brak tej właściwości. |
| `has_check` | bool | Opcjonalna obserwacja jednokolumnowego CHECK w schemacie v6. Każde `true` jest objęte `check_count` tabeli. |
| `null_fraction` | liczba zmiennoprzecinkowa. | Opcjonalny, obserwowany odsetek wartości null z zakresu `0.0` do `1.0`. Jeśli dostępna jest informacja o kardynalności, jest ona pobierana z zaokrąglonych, publicznych liczników tego bloku; w przeciwnym razie jest zaokrąglana niezależnie. Żadna mapa bitowa wartości null nie jest przechowywana. |
| `native_type` | string | Opcjonalny oczyszczony typ bazowy silnika, taki jak `varchar` lub `longtext`; bez identyfikatorów, elementów typu wyliczeniowego, wartości domyślnych i wyrażeń. Emitowany przez natywne kolektory MySQL i SQL Server. |
| `declared_max_chars` | int | Opcjonalna zadeklarowana pojemność znakowa. Dokładna dla wartości katalogowych PostgreSQL `character`/`character varying` oraz w domyślnym zrównoważonym/dokładnym trybie MySQL; zgrubnie zaokrąglana tylko z MySQL `--length-fidelity strict`. |
| `declared_max_bytes` | int | Opcjonalna zadeklarowana pojemność bajtowa. Dokładna w domyślnym zrównoważonym/dokładnym trybie MySQL; zgrubnie zaokrąglana tylko z `--length-fidelity strict`. |
| `length_semantics` | ciąg znaków | Wersja V7: Opcjonalna jednostka długości deklarowana: `characters`, `bytes`, `not-applicable` lub `unknown`. Pozwala to zachować semantykę Oracle CHAR w porównaniu do BYTE bez serializacji deklaracji. |
| `numeric_model` | ciąg znaków | Wersja V7 wymagała zamkniętej rodziny: `integer`, `fixed-decimal`, `unconstrained-decimal`, `decimal-float`, `binary-float`, `not-applicable` lub `unknown`. `not-applicable` oznacza znany typ nie numeryczny; `unknown` jest zarezerwowany dla typu numerycznego lub zdefiniowanego przez użytkownika, którego semantyka nie została sklasyfikowana. `decimal-float` zawiera dokładne wartości Oracle `FLOAT(p)` i nie jest liczbą zmiennoprzecinkową IEEE. |
| `numeric_precision` | int | Opcjonalna, dodatnia, zadeklarowana precyzja, ograniczona przez silnik i model źródłowy: dla Oracle `NUMBER` i SQL Server, precyzja dziesiętna do 38, dla Oracle `FLOAT(p)` do 126 bitów binarnych, dla MySQL, precyzja dziesiętna do 65, a dla PostgreSQL, precyzja numeryczna do 1000. |
| `numeric_scale` | int | Opcjonalna, podpisana, zadeklarowana skala, weryfikowana w odniesieniu do silnika źródłowego. Oracle `NUMBER` używa `-84..127`; PostgreSQL obsługuje jego szerszy, zależny od wersji zakres deklaracji, podczas gdy MySQL, SQL Server, Parquet i Avro wymagają nieujemnej skali, która nie jest większa niż precyzja. Ujemna skala Oracle/PostgreSQL i skala większa niż precyzja, jeśli silnik na to pozwala, są zachowywane. |
| `numeric_precision_radix` | ciąg znaków | `decimal` lub `binary`, gdy jest to wymagane przez model numeryczny. Oracle `FLOAT(p)` używa precyzji binarnej z dokładną wartością `decimal-float`; `BINARY_FLOAT` i `BINARY_DOUBLE` używają `binary-float`. |
| `numeric_unsigned`, `bit_width` | bool / int | Opcjonalna interpretacja liczb całkowitych, jeśli silnik źródłowy ją udostępnia. |
| `datetime_precision` | int | Opcjonalna, zadeklarowana przez silnik, precyzja ułamkowa date/time. |
| `charset`, `collation` | string | Opcjonalne oczyszczone metadane znakowe. MySQL emituje katalogowe nazwy zestawu znaków i sortowania. SQL Server emituje `utf-16le` dla `nchar`/`nvarchar`/`ntext`, `utf-8` dla strony kodowej 65001, `windows-N` dla stron kodowych Windows 1250–1258 lub `code-page-N` dla innej dodatniej katalogowej strony kodowej, a także katalogową nazwę sortowania. Są to fakty o kodowaniu i nazwy katalogowe, nigdy Twoje identyfikatory ani wartości. |
| `len_avg` | int | Próbkowana średnia liczba bajtów wartości o zmiennej długości. Domyślne względne przedziały mają około 3,2% maksymalnego błędu i dokładnie zachowują wartości do 32 bajtów; dokładne z `--length-fidelity exact --yes`; zgrubnie do najbliższych 10 tylko w trybie ścisłym. 0 = stała długość albo brak pomiaru. |
| `len_p95` | int | Próbkowany 95. percentyl z tymi samymi domyślnymi względnymi przedziałami; dokładny z `--length-fidelity exact --yes`; zgrubnie do najbliższych 100 tylko w trybie ścisłym. 0 = brak pomiaru. |
| `style` | string | Tylko Poziom 2. Jedna z wartości `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"` lub `"precompressed"`; puste, jeśli nie sklasyfikowano. `"precompressed"` jest emitowane wyłącznie dla istotnie dominującej bajtowo próbki wartości binarnych z rozpoznanymi sygnaturami standardowych kontenerów. Rodzina wykrytego kontenera celowo nie jest ujawniana. |
| `[tables.<id>.cols.<cid>.lob_storage]` | podtabela | Dowody dotyczące opcjonalnego przechowywania danych LOB w bazie danych w wersji V7: `storage_class` (`basicfile`, `securefile`, `external`, `unknown`), kompresja (`none`, `low`, `medium`, `high`, `not-applicable`, `unknown`), deduplikacja (`enabled`, `disabled`, `not-applicable`, `unknown`), opcjonalne flagi in-row/encrypted oraz widoczność (`full`, `partial`, `unknown`). Zewnętrzna zawartość wymaga, aby dwa mechanizmy kontroli przechowywania były ustawione na `not-applicable` i pomija flagi wewnątrz bazy danych. Żadna ścieżka ani nazwa segmentu nie są zachowywane. |
| `magnitude_min`, `magnitude_max` | int | Opcjonalne dziesiętne wykładniki ze znakiem w schemacie v6, ograniczające wielkość próbkowanych liczb innych niż NULL. Są emitowane z `has_negative`; dokładne wartości nigdy nie są serializowane. |
| `has_negative` | bool | Opcjonalna obserwacja znaku w schemacie v6, emitowana tylko z obiema granicami wielkości. |
| `time_span` | string | Opcjonalny próbkowany zakres daty/czasu w schemacie v6: `intraday`, `days`, `weeks`, `months`, `years` albo `decades`. |
| `time_recent_decade` | int | Dekada najnowszej próbkowanej daty/czasu w schemacie v6, emitowana tylko z `time_span` i zawsze podzielna przez 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tylko Poziom 2. Obecne dla próbkowanych kandydujących kolumn tekstowych/binarnych. Taki sam układ pól jak dla kompresji na poziomie tabeli, ale ograniczony do jednej zanonimizowanej kolumny. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Podsumowanie rozkładu próbkowanych wartości w schemacie v3. Zawiera tylko ograniczone i zaokrąglone liczności oraz częstotliwości. |

`numeric_model` jest autorytatywne w kwestii semantyki numerycznej. `type` zachowuje pisownię rodziny silników: rodzina `NUMBER` firmy Oracle jest uzupełniana przez `type = "number"`, a `FLOAT(p)` przez `"float"`, podczas gdy `native_type` zachowuje oryginalną, oczyszczoną deklarację.

### `[tables.<id>.cols.<cid>.cardinality]` (schemat v3)

Kiedy włączone jest pobieranie próbek wierszy, kolektor przechowuje w pamięci maksymalnie 8192 tymczasowych 64-bitowych odcisków cyfrowych dla każdej kolumny, oblicza zagregowane statystyki NDV/skew i usuwa te odciski. Ani wartości, ani odciski cyfrowe nie są serializowane. Blok zawiera elementy `measured`, `sample_rows`, `non_null_rows`, `observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`, `frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`, `sample_method`, `complete_source_read`, `sample_layout`, `sampled_with_bias` i `bias_reason`. `complete_source_read = true` to maszynowo czytelny dowód, że jedno określone stwierdzenie obejmowało całą widoczną populację danych źródłowych i zachowało tę kolumnę bez obcinania wartości. Sprawdzony, kompletny odczyt wiersza utrzymuje `sample_rows` w dokładnym zakresie wierszy w tabeli, nawet gdy limit komórki lub ograniczony bufor odcisków sprawia, że `complete_source_read` jest fałszywy; Dokładna zawartość tabeli jest już dostępna w `tables.<id>.rows`, więc to nie ujawnia żadnych dodatkowych informacji. `non_null_rows` jest najpierw zaokrąglane pod względem prywatności, a następnie `null_fraction` jest wyprowadzane z `(sample_rows - non_null_rows) / sample_rows`. Ułamek ten pozostaje więc dokładnie zgodny z publicznymi danymi i może zostać usunięty z niezależnej siatki wartości 0,005 bez ujawniania żadnych innych informacji. Dokładne wartości zerowe i wszystkie punkty końcowe, w których nie występują wartości NULL, są zachowywane; Zestaw danych mieszanych utrzymuje dodatnią, niezerową populację i pozostaje poniżej wartości `sample_rows`. Wersja Mixed `non_null_rows` wykorzystuje tę samą siatkę liczbową, która jest stosowana do innych wartości kardynalności. W przypadku występowania dużej ilości powtórzeń, liczba ta może być nawet o jeden pełny przedział poniżej rzeczywistej, zachowanej populacji (na przykład `9,728` zamiast `9,999`). To nie jest dokładne zliczenie. Dokładny, zawierający wszystkie wartości nie-NULL, wynik wyraźnie wskazuje, że w zachowanych wierszach nie zaobserwowano wartości NULL, natomiast każda zaobserwowana wartość NULL utrzymuje liczbę poniżej `sample_rows`. Unikalne wartości i liczby wystąpień zachowują swoje zdefiniowane właściwości dotyczące prywatności, nawet gdy są ograniczone do określonej grupy danych. Dla baz danych źródłowych, niepełne odczyty ograniczają `sample_rows` do zaokrąglonego szacunku liczby wierszy w katalogu; Dla formatów Parquet i Avro, dokładna liczba wierszy w stopce stanowi limit. W obu ścieżkach `sample_rows` to dokładna, zachowana liczba wierszy, która została już ujawniona przez blok kompresji na poziomie tabeli, a konkretnie przez `sample_rows`, chyba że ten limit jest niższy. Nigdy nie powinno się go modyfikować w taki sposób, aby sugerować pełne pokrycie. Skracanie wartości danych pozwala zachować rzetelność i wiarygodność danych oraz częstotliwość występowania, i nigdy nie powinno prowadzić do zawyżania tych wartości w stosunku do rzeczywistej populacji tabeli. Nie należy wnioskować o kompletności na podstawie analizy `sample_method`. `sample_layout` jest opcjonalnym, maszynowo czytelnym wyliczeniem. Aktualna wygenerowana wartość to `primary-key-range-windows`; Brak dostępności oznacza, że nie jest dostępna żadna umowa dotycząca kolejności przetwarzania. Nie należy wnioskować o znaczeniu na podstawie analizy pola `sample_method`, które jest czytelne dla człowieka.

Liczby i ułamki są zaokrąglane w celu ochrony prywatności, w miarę potrzeby. Statystyki opisują gęstość duplikatów, nierównomierne rozłożenie wartości oraz skończone zakresy. Nie zawierają one próbek, ale charakterystyczne rozkłady mogą identyfikować obciążenie; nie należy traktować ich jako nieodwracalnych ani jako dowodu na to, że znaczenie biznesowe nie może być wywnioskowane z zewnętrznej wiedzy. Ograniczone stwierdzenie może udowodnić widoczną populację wierszy, nie udowadniając jednocześnie, że każda próbkowana komórka została w pełni zachowana. Jeśli limit komórki po stronie serwera obcina kolumnę, jej kardynalność pozostaje ograniczonym, obciążonym oszacowaniem, nawet jeśli liczba wierszy w tabeli jest rejestrowana z pełnego, ograniczonego odczytu; niezakłócone kolumny mogą nadal zawierać informacje o kardynalności pochodzącej z pełnego odczytu.

### `[tables.<id>.cols.<cid>.compression]` (tylko Poziom 2)

Kompresja na poziomie kolumn jest generowana tylko dla ograniczonych text/binary kandydatów, gdy używana jest opcja `--measure-compression --yes`. Dostarcza ona szacunkową wartość kompresji dla każdego kolumn.

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
sample_method = "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "server_side_cell_cap"
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "blueprint-compression-probe-v2"
```

Żadne próbkowane wartości kolumn nie są zapisywane w pliku Blueprint.

Dla kolumn binarnych ta sama ograniczona próbka Poziomu 2 może emitować zgrubny
profil `style = "precompressed"`. Rozpoznawanie zachodzi wyłącznie na granicach
próbkowanych wartości i wymaga istotnej, dominującej bajtowo obserwacji.
Blueprint nie analizuje ani nie dekompresuje wartości, nie zachowuje jej
sygnatury i nie rozróżnia obrazów, archiwów, skompresowanych multimediów,
zaszyfrowanych i losowych ładunków poza tą jedną etykietą o wysokiej pewności.
Kodowania tekstowe i base64 nadal są klasyfikowane według stylu tekstu i nie są
traktowane jako wstępnie skompresowane kontenery binarne.

## `[tables.<id>.idxs.<iid>]`

Identyfikator to `idx-N`, gdzie `N` to pozycja indeksu (liczona od 1) w obrębie tabeli, posortowana według HMAC-SHA256 nazwy indeksu, oddzielonego znakiem specjalnym. Schemat wersji 7 wymaga zbioru ciągów `idx-1` do `idx-N` dla każdej tabeli; zero, wiodące zera, luki oraz sufiksy nie będące cyframi są nieprawidłowe.

| Pole | Typ | Wartości |
|---|---|---|
| `type` | string | Znormalizowana rodzina metod indeksowania, taka jak `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"` lub `"other"`. Nazwy metod rozszerzeń i niestandardowych nie są emitowane. |
| `primary` | bool | Opcjonalne; emitowane jako `true` dla indeksów klucza głównego. W przeciwnym razie pominięte/false. |
| `unique` | bool |  |
| `cols` | array of int | numery porządkowe uczestniczących kolumn, w kolejności kolumn indeksu |
| `prefix_lengths` | array of int | Opcjonalne długości prefiksów indeksu MySQL wyrównane z `cols`; zero oznacza pełną kolumnę. Domyślnie dokładne; zaokrąglane w dół tylko z `--length-fidelity strict`. |
| `include_cols` | array of int | Opcjonalne; numery porządkowe kolumn INCLUDE niebędących kluczami, jeśli silnik źródłowy je udostępnia. |
| `expression` | bool | Opcjonalne; true, gdy istnieje materiał klucza w postaci wyrażenia/funkcji, którego nie można przedstawić jako prostych numerów porządkowych kolumn. |
| `filtered` | bool | Opcjonalne; true dla indeksów filtrowanych/częściowych. |
| `descending` | bool | Opcjonalne; true, gdy dowolna kolumna klucza jest jawnie malejąca. |
| `partitioning` | ciąg znaków | Opcjonalne fizyczne partycjonowanie w wersji V7: `none`, `local`, `global` lub `unknown`. |
| `visibility` | ciąg znaków | Opcjonalna widoczność źródła w wersji V7: `visible`, `invisible` lub `unknown`. |
| `state` | ciąg znaków | Opcjonalne stany operacyjne w wersji V7: `usable`, `unusable`, `in-progress`, `failed` lub `unknown`. |
| `prefix_distinct_counts` | array of int | Szacowana w schemacie v3 liczba odrębnych krotek dla każdego prefiksu klucza od jednej do N kolumn. Zero oznacza brak danych dla danego prefiksu. |
| `cardinality_sample_method` | string | Ograniczone pochodzenie `prefix_distinct_counts`; iloczyny wywnioskowane są jawnie oznaczone i nie są przedstawiane jako bezpośrednie próbki krotek. |

## `[tables.<id>.compression]` i `[tables.<id>.cols.<cid>.compression]` (tylko Poziom 2)

Tekst ten jest wyświetlany tylko wtedy, gdy plik został wygenerowany przy użyciu `--measure-compression --yes`. Blok na poziomie tabeli mierzy neutralną, kolumnową projekcję pełnej próbki i pozostaje wiarygodnym wskaźnikiem dla szacunków transferu całych tabel. Bloki na poziomie kolumn są tworzone na podstawie tych samych wierszy próbki, po jednej kolumnie, i pokazują, które kolumny dobrze się kompresują, nie ujawniając przy tym wartości z próbek. Nie powodują one dodatkowych odczytów z bazy danych.

Tabele PostgreSQL, na których obowiązują aktywne mechanizmy bezpieczeństwa na poziomie wierszy, w tym tabele potomne (dziedziczone lub partycjonowane), których polityka rodzica zostałaby pominięta przez bezpośrednie zapytanie potomne, oraz tabele SQL Server, na których obowiązuje aktywny filtr bezpieczeństwa, nie są uwzględniane w próbkach. Ich informacje o katalogu są zachowywane, a w raportach z działania `DBP1407W` zamiast ekstrapolować podzbiór filtrowany przez politykę jako całość tabeli, prezentowane są dane dotyczące tych konkretnych tabel.

| Pole | Typ | Dokładność |
|---|---|---|
| `measured` | bool | zawsze `true`, jeśli blok jest obecny |
| `sample_rows` | int | exact |
| `sample_bytes` | int | rozmiar bufora próbki w pamięci, **podzielony na przedziały**: najbliższe **64 KiB** poniżej 1 MiB, najbliższe **1 MiB** poniżej 1 GiB, najbliższe **100 MiB** powyżej. Bajty nigdy nie są zapisywane na dysku. Podział na przedziały usuwa ukryty kanał najmniej znaczących bitów dla każdej tabeli, który w przeciwnym razie ujawniałaby dokładna wartość `buf.len()`. |
| `sample_method` | string | właściwy dla silnika opis ograniczonego próbkowania, na przykład `"TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`, `"LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"` lub `"SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)"` |
| `sampled_with_bias` | bool | true, jeśli próbka nie jest równomierna, na przykład w przypadku ścieżki awaryjnej używającej tylko LIMIT |
| `bias_reason` | string | Kiedy `sampled_with_bias = false`, to pole jest puste. W przeciwnym razie zawiera znacznik, taki jak `"unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"`. |
| `ratio_zstd_3` | float | zaokrąglony do najbliższych **0,05**, zgodnie z kontraktową polityką pomiaru zstd poziomu 3. Zmierzony na bajtach zakodowanych za pomocą `sample_encoding`. |
| `ratio_zstd_19` | liczba zmiennoprzecinkowa. | Nie zostało napisane w tej wersji; może pojawić się w plikach z wcześniejszych wersji. |
| `ratio_stddev` | float | zaokrąglone do najbliższych **0,05**, odchylenie standardowe współczynników poziomu 3 dla ograniczonych ramek próbnika tabeli. Bloki odwzorowania na poziomie kolumn obecnie emitują `0.0`, ponieważ są pomocniczymi wskazówkami entropii, a nie modelem wariancji. |
| `sample_encoding` | ciąg znaków | Identyfikator kodowania na poziomie bajtów oraz polityki kompresji sesji używanej do pomiaru. Bloki tabel w PostgreSQL używają `"blueprint-columnar-transfer-probe-v2"`. MySQL i SQL Server używają `"blueprint-columnar-transfer-probe-v3"`, które dodatkowo zrzucają dane w granicach bloków o rozmiarze 256 KiB. Obciążenia `nvarchar`/`nchar`/`ntext` w SQL Server zachowują natywne rozkłady bajtów UTF-16LE, a `varchar`/`char`/`text` zachowują ich próbkowaną szerokość bajtu, a pole `charset` identyfikuje kod stronę katalogu. Wersja V1 jest akceptowana jako dane wejściowe. Bloki dla poszczególnych kolumn używają `"blueprint-compression-probe-v2"`. Wartości, które są mierzone przy użyciu różnych wartości `sample_encoding`, nie są porównywalne. |

PostgreSQL używa wersji v2, a MySQL i SQL Server używają wersji v3; porównuj współczynniki tylko w obrębie jednego kodowania.

### Kodowanie bajtowe `blueprint-compression-probe-v2`

Próbnik Poziomu 2 łączy wiersze lub próbkowane wartości kolumn w buforze w
pamięci przy użyciu poniższego formatu, a następnie uruchamia na nim zstd na
poziomie 3. Bufor jest odrzucany. Blueprint zachowuje wyłącznie udokumentowane
zagregowane pola kompresji, gęstości NULL, liczności/częstotliwości, długości i stylu.

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

Znaczniki typów są częścią kontraktu próbnika i nie zostaną przenumerowane bez
nowego, wersjonowanego identyfikatora próbnika.

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

### Kodowanie bajtowe `blueprint-columnar-transfer-probe-v1`, `v2` i `v3`

Współczynniki tabel z działających baz danych przekształcają te same ograniczone
próbki kolumn v2 w neutralne ramki po 1000 wierszy. Każda ramka ma wersjonowany
nagłówek próbnika, a dla każdej kolumny: numer porządkowy, jeden znacznik typu,
czterobajtową długość dla każdego wiersza, a następnie ciągłe bajty ładunku
kolumny. Długość `0xffffffff` oznacza NULL. Reprezentacja bajtowa jest wspólna
dla wszystkich trzech wersji. V1 kompresował połączoną sekwencję ramek w jednej
operacji zstd poziomu 3 z zadeklarowanym rozmiarem wejścia. V2 przekazuje ramki
przez jeden trwały kontekst zstd poziomu 3 i opróżnia go po każdej ramce. V3
zachowuje ten kontekst i neutralną reprezentację grup wierszy, ale opróżnia go
także na każdej granicy fragmentu kompresji sondy o rozmiarze 256 KiB w grupie.
MySQL i SQL Server używają v3; v2 pozostaje bieżącym pomiarem PostgreSQL.
Tekst Unicode SQL Server jest mierzony jako UTF-16LE.
Wąski tekst SQL Server zachowuje źródłową szerokość bajtową i
zapisuje zamknięty, oczyszczony zestaw znaków wyprowadzony ze strony kodowej
sortowania. Wyniki zewnętrznych grup wierszy dostarczają obserwacji
`ratio_stddev`. Wersjonowane znaczniki zapobiegają cichemu reinterpretowaniu
jednej polityki ramek lub opróżniania jako innej.

Ta reprezentacja modeluje ogólne, istotne dla kompresji właściwości kolumnowego
transferu masowego. Nie jest zapisem protokołu bazy danych, formatem
transportowym migracji ani zakodowanym eksportem danych. Bajty próbek pozostają
wyłącznie w pamięci i są usuwane po wyprowadzeniu pomiarów zagregowanych.

### Granice dokładności

`ratio_zstd_3` opisuje obiekt o nazwie `sample_encoding`; nie jest to zapis protokołu bazy danych ani danych przesyłanych podczas migracji. Zestaw testów w tym repozytorium weryfikuje deterministyczne kodowanie, ograniczoną próbkowanie i serializację, ale nie gwarantuje uniwersalnego, procentowego błędu w stosunku do wszystkich ścieżek ekstrakcji, niezależnie od silnika bazy danych.

Przed użyciem tego współczynnika do podjęcia ważnej decyzji dotyczącej wydajności, sprawdź go w odniesieniu do reprezentatywnych danych źródłowych i planowanego mechanizmu ekstrakcji. Zapisz metodę porównania, wielkość próbki, skrót binarny, wersję silnika oraz zaobserwowany błąd wraz z uzyskanym planem. Podstawowa relacja to `compressed_bytes ≈ sample_bytes / ratio_zstd_3` w odniesieniu do rozkładu bajtów generowanego przez zarejestrowane kodowanie.

## `[fk_edges]`

Opcjonalna tabela wbudowana, w której każdy klucz jest identyfikatorem `table-NNN` odwzorowanym na listę krawędzi. Schemat v3 zachowuje numery porządkowe kolumn nadrzędnych, akcje referencyjne, tryb dopasowania, odraczalność, stan walidacji/zaufania oraz opcjonalne, ograniczone i pozbawione nazw podsumowanie relacji. Krawędzie są sortowane według celu, a następnie listy kolumn.

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

Opcjonalny blok `statistics` zapisuje próbkowane lub wywnioskowane wartości `non_null_rows`, `distinct_parent_values`, `parent_coverage_fraction`, fanout p50/p95/p99/max oraz `orphan_rows`, wraz z polami pochodzenia i obciążenia próbki. Zweryfikowane ograniczenia źródłowe oznaczają zero sierot. Estymaty złożone wyprowadzone z próbek poszczególnych kolumn są jawnie oznaczone jako wywnioskowane.

## `[artifact_inventory]` (od wersji schematu v4; wymagane w wersji v7)

Wersja schematu v7 wykorzystuje niezależnie wersjonowany kontrakt `dbwarp-blueprint-artifacts/v2` do opisywania obiektów innych niż tabele, bez serializacji nazw lub definicji źródła. Starsze wersje schematu zachowują kontrakt v1. Wersja v7 zawsze generuje ten blok: `--artifact-detail none` wyraźnie rejestruje niezażądzony spis zasobów bazy danych, podczas gdy źródła plików strukturalnych generują wyraźny spis "nie dotyczy". Brakujący blok nigdy nie zostanie błędnie uznany za zweryfikowany, pusty katalog.

Domyślne `--artifact-detail summary` emituje `object_count`,
`external_prerequisite_count`, `counts_by_kind` i
`counts_by_external_class`. `graph` dodaje anonimowy rekord obiektu dla każdego
artefaktu oraz krawędzie zależności. `analyzed` dodaje ograniczone rekordy
`dbwarp-language-feature-census/v1` wyprowadzone tymczasowo z dostępnych
definicji. `graph` i `analyzed` wymagają jawnego `--yes`, ponieważ topologia
grafu może identyfikować aplikację.

`object_count` to liczba rekordów artefaktów generowanych przez kolektor, a nie liczba wierszy zwracanych przez żaden konkretny, natywny katalog. Dlatego też, pakiet lub typ może przyczynić się oddzielnymi rekordami specyfikacji, treści i elementów. Natywny obiekt, który pojawia się w więcej niż jednym katalogu, nadal jest reprezentowany przez jeden rekord: na przykład, wiersze wyzwalaczy Oracle z katalogów wyzwalacza i źródła są łączone przez ich natywną identyfikację obiektu, a wiersze źródłowe wzbogacają, a nie duplikują rekord wyzwalacza.

Pakiety i typy obiektów Oracle używają tej samej struktury rekordu: rekordu `specification`, rekordu `body` powiązanego jako jego implementacja oraz jednego rekordu `package_member` procedure/function dla każdego elementu katalogu, gdzie specyfikacja jest rodzicem. Tylko ciało zawiera połączony tekst źródłowy i analizę językową; elementy zachowują informacje o katalogu, ale używają analizy definicji, która nie jest stosowalna. Zapobiega to analizatorowi leksykalnemu, który udaje, że może podzielić kod źródłowy pakietu na ciała elementów.

Dowody na poziomie inwentarza obejmują:

| Pole | Wartości / reguła |
|---|---|
| `detail` | `none`, `summary`, `graph` lub `analyzed` |
| `scope` | Wersja V7: `all-visible-schemas`, `selected-schemas`, `structured-source` lub `unknown`; musi ona być zgodna z informacjami dotyczącymi wyboru schematu zawartymi w innym miejscu tego pliku. |
| `visibility` | `full`, `privilege_filtered` lub `unknown` |
| `inventory_complete` | Może być prawdziwe tylko przy pełnej widoczności, bez nieczytelnych katalogów i zadeklarowanych niezamodelowanych rodzin |
| `dependencies_complete` | Może być prawdziwe tylko wtedy, gdy modelowane katalogi zależności były czytelne |
| `requirements_complete` | Agregat V7: prawda tylko przy pełnym pokryciu populacji oceny w wybranym zakresie i `requirement_status = complete | not_applicable` dla każdego wyemitowanego artefaktu; pominięcie oznacza fałsz, a pusta lista wymagań nie dowodzi kompletności |
| `analysis_complete` | Może być prawdziwe tylko dla poziomu analyzed i gdy każda wyemitowana analiza jest kompletna |
| `catalogs_read` | Zamknięte standardowe etykiety pomyślnie sprawdzonych katalogów silnika |
| `catalogs_unreadable` | Etykiety katalogów, które nie powiodły się; każdy taki przypadek uniemożliwia zapewnienie kompletności danych dla danego katalogu, podczas gdy niezależne dowody spełnienia wymagań dla poszczególnych obiektów mogą pozostać kompletne. |
| `catalogs_not_applicable` | Etykiety katalogów w wersji V7 okazały się nieodpowiednie; są odrębne od katalogów, które można odczytać, oraz od tych, których nie można odczytać. |
| `families_not_inventoried` | Znane rodziny obiektów, które nie zostały uwzględnione w tej wersji |

### `[artifact_inventory.complexity]` (schemat v7)

Blok `dbwarp-blueprint-artifact-complexity/v1` to agregatowa ocena oparta wyłącznie na anonimowym spisie artefaktów. Jest on nieobecny w szczegółach `none` i `summary`, a wymagany w szczegółach `graph` i `analyzed`, gdzie jego obecność oznacza, że próba oceny została podjęta. Awaria obliczeń powoduje uzyskanie nieznanego wyniku z domyślnym zachowaniem "fail-closed" zamiast przerwania działania Blueprint.

Pola najwyższego poziomu są stałe:

| Pole. | Wartości / reguła. |
|---|---|
| `contract` | `dbwarp-blueprint-artifact-complexity/v1` |
| `assessor_version` | `1` |
| `scope` | Musi dokładnie odpowiadać `artifact_inventory.scope`. |
| `population_policy` | `exclude-known-engine-generated-and-secondary`; brakujące flagi pozostają aktywne, a tymczasowe obiekty pozostają dostępne. |
| `assessment_population_complete` | Prawda tylko wtedy, gdy wszystkie obiekty spełniające kryteria określone w polityce zakresu są znane; pominięcie oznacza fałsz, a to stwierdzenie jest niezależne od szerszego pola `inventory_complete`. |
| `eligible_object_count` | Obiekty, które są oceniane przez politykę. |
| `fully_assessed_object_count` | Każdy parametr jest albo znany, albo udowodniony `not-applicable`. |
| `partially_assessed_object_count` | Przynajmniej jeden istotny parametr jest znany, a przynajmniej jeden jest nieznany. |
| `unassessed_object_count` | Brak znanych odpowiednich parametrów. |
| `excluded_object_count` | Obiekty wykluczone przez obowiązującą politykę. |
| `analyzer_version` | Analizator używany przez narzędzie do zbierania danych w wersji 7: `lexical-v2` lub `not-applicable` w trybie graficznym. |
| `analysis_spans` | Posortowane, unikalne, zamknięte zakresy obecne w kwalifikujących się danych spisowych: `executable-body`, `not-applicable` lub `unknown`; puste w trybie graficznym. |
| `dialects` | Posortowane, unikalne, zamknięte tokeny dialektalne występujące w dostępnych danych spisowych. |
| `grammar_profiles` | Posortowane, unikalne profile gramatyczne występujące w dostępnych danych spisowych. |
| `overall_band` | `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable` lub `unknown`. |
| `overall_score` | Nie zostało napisane przez tę wersję. |
| `limitations` | Posortowane, zamknięte powody, opisane poniżej. |

Oba równania populacyjne wykorzystują arytmetykę z kontrolą błędów:

```text
artifact_inventory.object_count = eligible_object_count + excluded_object_count
eligible_object_count = fully_assessed_object_count
                      + partially_assessed_object_count
                      + unassessed_object_count
```

`dimensions` zawiera dokładnie `volume`, `control_flow`, `feature_breadth`, `entanglement`, `environment_coupling`, `opacity` i `dialect_coupling`. Każdy wymiar ma zamknięty zakres `band`, wartość `coverage` (`complete`, `partial`, `not-applicable` lub `unknown`) oraz jeden stały histogram. Wymiar `volume` `band`, podobnie jak pozostałe wymiary, używa `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable` lub `unknown`. Jego histogram używa kluczy rozmiaru `0`, `1-255`, `256-1k`, `1k-4k`, `4k-16k`, `16k-64k` i `64k+`. Pozostałe sześć histogramów używa kluczy liczbowych `0`, `1`, `2-4`, `5-8`, `9-16`, `17-32` i `33+`. Każdy histogram ma również koszyki `not_applicable` i `unknown`. Dla każdego wymiaru, sprawdzanie arytmetyczne wymaga:

```text
eligible_object_count = assessed evidence-band counts
                      + not_applicable
                      + unknown
```

Pokrycie jest więc określane dla każdej wymiaru, a nie dla pojedynczego obiektu na najwyższym poziomie. Wynik spisu `not_applicable` jest uznawany za dowód i przyczynia się do "koszyka" `not_applicable` danego wymiaru; nie oznacza to, że obiekt nie został oceniony. Liczby obiektów na najwyższym poziomie fully/partially/unassessed to podsumowanie: wszystkie obiekty oznaczone jako "nie dotyczy" są w pełni ocenione, "częściowo" oznacza, że co najmniej jeden wymiar jest znany, a inny jest nieznany, a "nieoceniony" oznacza, że żaden wymiar nie jest znany.

Częściowa zmienna jest oceniana jako dolna i górna granica. Jej `band` wynosi `unknown`, chyba że znana dolna granica jest już równa `very-high`, ponieważ każda nieznana obserwacja może znajdować się w najwyższej kategorii. Zapobiega to prezentowaniu przez częściowy histogram jego obserwowanej dolnej granicy jako ostatecznego wyniku.

`external_binary` to stan widoczności definicji, a nie flaga wykluczająca. Wtyczki zainstalowane w środowisku, biblioteki CLR, obiekty Java i zewnętrzne biblioteki pozostają elementami, które można przenieść, i zazwyczaj przyczyniają się one do nieznanych danych zależnych od definicji. Tylko wyraźna flaga `generated_by_engine = true` wyklucza obiekt dostarczony przez silnik w wersji oceniacza v1.

Histogramy są celowo jednowymiarowe. Tabele krzyżowe według rodzaju, cechy, schematu lub jakiejkolwiek innej cechy nie są częścią umowy. Dokładne liczby nie dostarczają żadnych informacji, które nie byłyby zawarte w serializowanym, obiektowym spisie w trybie analizy, a stała struktura zapobiega publikowaniu oficjalnego narzędzia do identyfikacji bazy danych.

Powody ograniczeń to: `definition-analysis-not-requested`, `definitions-withheld`, `unsupported-dialect`, `wrapped-source`, `graph-incomplete`, `requirements-incomplete`, `outside-selected-scope`, `computation-limit` i `computation-failed`. `requirements-incomplete` oznacza, że zbieranie wymagań nie jest kompletne. Obiekty o statusie wymagań `partial` lub `unavailable` generują nieznane, a nie zerowe, obserwacje dotyczące zależności od środowiska i dialektu; niezależnie kompletne obiekty pozostają oceniane. `unsupported-dialect` oznacza, że definicja została uzyskana, ale jej język lub dialekt nie ma obsługiwanego analizatora; nie jest ona ani pomijana, ani celowo ukrywana. Szczegółowość grafu wykorzystuje `definition-analysis-not-requested`; nie może ona twierdzić, że występuje ograniczenie odczytu definicji, ponieważ nie próbowano przeprowadzić takiego odczytu.

Nieznane dowody są ograniczane niezależnie dla każdego dotkniętego wymiaru. Całkowity zakres jest emitowany tylko wtedy, gdy dolna i górna ocena są zgodne. Pełna, pusta i kwalifikująca się populacja to `not-applicable`, a nigdy `trivial`. Tryb graficzny zawsze używa zakresu `unknown` dla populacji niepustej, ponieważ nie odczytuje definicji wymaganych przez ogólną ocenę. Brakujące krawędzie grafu powodują, że dotknięte dowody powiązania są nieznane. `computation-limit` nie jest zapisywane w tej wersji. Nieoczekiwana awaria obliczeń rejestruje `computation-failed`, zachowuje pełny inwentarz artefaktów i oznacza, że ocena agregowana jest domyślnie wyłączona, zamiast tłumić wynik Blueprint.

`assessment_population_complete`, a nie ogólne `artifact_inventory.inventory_complete`, decyduje o tym, czy uzyskany, ograniczony wynik może być wiarygodny. Jeśli grupa obiektów podlegających ocenie jest niepełna, ogólny zakres wynosi `unknown`, chyba że znana dolna granica jest już równa `very-high`; nie zakłada się żadnej skończonej górnej granicy dla obiektów, które mogą być niewidoczne.

Całkowicie zawarte obiekty przyczyniają się o `unknown` do wartości krycia; nie są one pomijane w histogramie krycia tylko dlatego, że nie można było przeprowadzić pełnego spisu. Prezentuj wartość krycia obok jej zasięgu, aby niski obserwowany zakres obszarów krytych nie mógł ukryć dużej, nieznanej populacji.

`unsupported-dialect` pozostaje wyraźnym ograniczeniem, ponieważ spis może wskazać dialekt i raportować `unavailable`, ale nie ma statusu `unsupported`. Jest on generowany tylko wtedy, gdy dostępna jest definicja, a zarejestrowany dialekt nie jest obsługiwany przez wskazany analizator. Inne ograniczenia definicji są również generowane na podstawie widoczności definicji, statusu spisu i dostępnych dowodów, a nie są utrzymywane jako niezależne stwierdzenie.

Uprawnienia do porównania są obliczane na podstawie plików z zakresu umowy dotyczącej złożoności, wersji narzędzia oceniającego, wersji analizatora, dokładnego zakresu analizy, zestawów dialektów i profili gramatycznych, zakresu oraz polityki populacji. Ustawienie homogeneous/mixed o niskiej precyzji nie jest serializowane, ponieważ różne, mieszane zestawy niekoniecznie są porównywalne.

Złożoność jest zawsze specyficzna dla źródła danych. Zbiór zachowuje ocenę każdego podrzędnego komponentu "Blueprint" i nigdy nie tworzy poziomu złożoności dla całego zbioru, ani histogramu obejmującego różne silniki, wersje analizatorów, dialekty lub profile gramatyczne.

Identyfikatory poszczególnych obiektów mają postać `<kind>-NNN`, takie jak `view-001`, `package-002` lub `procedure-003`. Wersja V7 rozpoznaje powszechne rodziny obiektów, a także pakiety Oracle, obiekty harmonogramu, połączenia baz danych, katalogi, biblioteki, obiekty Java, operatory, typy indeksów, domeny, adnotacje oraz grafy właściwości, a także typy `queue` i `edition` niezależne od silnika. Minimalna wartość to trzy cyfry, uzupełniane zerami, a każdy typ ma swój własny, gęsty zbiór wartości początkowych od `001`; szerokość rośnie powyżej 999. Rekord zawiera tylko zamknięte tokeny kind/subkind/tier, anonimowe identyfikatory schema/parent, tryb definicji visibility/security, opcjonalne flagi ważności i katalogu, zamknięte pokrycie wymagań, opcjonalne zewnętrzne wymagania oraz opcjonalny spis języków. Rodzicem może być anonimowa tabela lub inny artefakt, więc hierarchia pakietów do procedur może być zachowana bez nazw; grafy rodziców muszą być acykliczne.

Wersja V7 używa jednego, zamkniętego słownika `subkind` we wszystkich silnikach:

```text
ordinary, other, materialized, integer_sequence, stored_procedure,
stored_function, scalar_function, inline_table_function, table_function,
user_defined_aggregate, table_trigger, ddl_event_trigger, before_insert,
before_update, before_delete, after_insert, after_update, after_delete,
generated_column, column_default, default_constraint, check_constraint,
row_security, rewrite_rule, legacy_rule, enum, domain, composite, range,
alias_type, table_type, clr_type, clr_procedure, clr_scalar_function,
clr_table_function, clr_aggregate, clr_trigger, clr_assembly,
server_extension, loadable_udf, foreign_data_wrapper_server, foreign_table,
federated_table, external_table, external_data_source, external_file_format,
logical_replication_publication, logical_replication_subscription,
full_text_catalog, partition_scheme, partition_function, tablespace, filegroup,
database_certificate, symmetric_key, asymmetric_key, column_master_key,
column_encryption_key, database_scoped_credential, linked_server,
enabled_event, disabled_event, enabled_agent_job, disabled_agent_job,
database_synonym, specification, body, package_member, public, private,
java_source, java_class, java_resource, external_library,
user_defined_operator, domain_indextype, scheduler_job, scheduler_program,
scheduler_schedule, scheduler_chain, advanced_queuing, service_broker
```

Wersja V7 zastępuje niejednoznaczną listę zależności v1 uporządkowanym, typowanym zestawem `relationships`. Rodzaje relacji rozróżniają wywołania, odczyty, zapisy, odniesienia table/object, własność wyzwalaczy, implementację, fizyczne rozmieszczenie, bezpieczeństwo, użycie rozszerzeń, zewnętrzne binaries/services i zdalne database/server użycie. Każda relacja rejestruje zamknięty token dowodowy (`catalog-confirmed`, `dependency-confirmed`, `syntax-confirmed`, `lexical-hint` lub `unresolved`). `dependency_edge_count` musi dokładnie odpowiadać wygenerowanemu grafowi.

`requirements` Należy używać tokenów specyficznych dla silnika i ograniczonej liczby tokenów. Identyfikują one wymagania dotyczące kompatybilności, takie jak źródło otoczone przez Oracle, złożony trigger, stan pakietu, dynamiczne zapytania SQL, autonomiczna transakcja, pipelined/parallel/aggregate procedura, biblioteka zewnętrzna, link do bazy danych, indeks domenowy, harmonogram, object/collection/spatial/vector typ, obiekt Java lub graf właściwości. Stanowią one jedynie dowód planowania.

Wymagania pochodzą z ograniczonego katalogu faktów lub dedykowanej, świadomej działania silnika, weryfikacji składni. Ogólna analiza leksykalna nigdy nie generuje wymagania specyficznego dla danego silnika. Każdy graf schematu w wersji 7 lub analizowany artefakt zawiera `requirement_status = complete | partial | unavailable | not_applicable`. `complete` określa, że lista jest wyczerpująca dla tego artefaktu; `not_applicable` określa, że model wymagań nie ma zastosowania i dlatego zabrania zapisów dotyczących wymagań i zewnętrznych zależności. `partial` i `unavailable` sprawiają, że obserwacje dotyczące środowiska i dialektu tego obiektu są nieznane. `partial` oznacza, że co najmniej jedno źródło faktów zakończyło się sukcesem bez pełnego pokrycia; `unavailable` oznacza, że żadne źródło wymagań nie ustanowiło użytecznego pokrycia i dlatego nie może zawierać znanych wymagań ani dowodów dotyczących zewnętrznych zależności. Takie dowody wymagają `partial`. Pozwala to na lokalną degradację jednego niedostępnego obiektu, zamiast usuwania przydatnego pokrycia dla reszty systemu.

Poziom zapasów `requirements_complete` to zagregowane stwierdzenie. Może być prawdziwe tylko wtedy, gdy każdy wygenerowany artefakt jest `complete` lub `not_applicable` i populacja podlegająca ocenie jest kompletna dla wybranego zakresu; może pozostać fałszywe nawet wtedy, gdy każdy artefakt jest `complete`. Nigdy nie należy interpretować pustej tablicy `requirements` jako zerowego powiązania, chyba że status tego artefaktu to `complete`.

`unresolved_relationships` to mapa ograniczona, która przypisuje powody do liczników. Rozróżnia ona odwołania zdalne i międzybazowe, granice wybranych schematów, ukryte cele uprawnień, zaszyfrowane lub pominięte definicje, dynamiczne zapytania SQL, niejednoznaczne powiązania, brakujące lub niekompletne identyfikatory natywne, niemożliwe do zmodelowania rodziny celów oraz nieznane przypadki. Pełne dowody zależności wymagają, aby ta mapa była pusta. Nazwy obiektów źródłowych, tekst SQL, podmioty, punkty końcowe, poświadczenia, klucze, certyfikaty i pliki binarne nie są polami w umowie.

Wymagania zewnętrzne zapisują zamkniętą `class`, zakres wdrożenia, konieczność
użycia nieprzechwyconych materiałów binarnych/sekretów/punktów końcowych oraz
ograniczoną kategorię zgodności. Ich liczba jest dowodem do planowania migracji,
a nie deklaracją, że DBWarp może je automatycznie udostępnić lub przetłumaczyć.

Wersje językowe V7 używają kodów `analyzer_version = "lexical-v2"` i `analysis_span`. Analizator otrzymuje tylko wykonywalną lub deklaratywną treść, wykluczając zewnętrzny mechanizm tworzenia, identyfikator, sygnaturę, deklarację zwracanych wartości oraz opcje modułu. Informacje z nagłówka pozostają wymaganiami katalogu lub flagami. Kolektor, który nie może bezpiecznie wyizolować treści, rejestruje kod `analysis_span = "unknown"` oraz brakujące dowody zamiast analizować zewnętrzny mechanizm. Sprawdzona, nieodpowiednia definicja używa kodu `analysis_span = "not-applicable"`. Brakujące informacje o zakresie są interpretowane ostrożnie jako `unknown`; nigdy nie są wnioskowane z silnika ani rodzaju obiektu. Obsługiwana definicja analizowana przez tę implementację leksykalną używa kodu `status = "partial"`; brakujące lub nieobsługiwane informacje o definicji używają kodu `unavailable`, a sprawdzony, nieodpowiedni obiekt może używać kodu `not_applicable`. Liczby, rozmiary, zagnieżdżenia, złożoność oraz wartości obszarów nieprzezroczystych to zakresy, a nie dokładne odciski źródłowe. Funkcje są wybierane z zamkniętego słownika. Analizator usuwa komentarze, literały i cytowane identyfikatory; nie jest to parser, wiązacz semantyczny ani gwarancja powodzenia tłumaczenia.

Zabezpieczony obiekt PL/SQL nigdy nie jest dowodem na istnienie kodu wykonywalnego. Kolektor oznacza go jako zaszyfrowany i wyłącza możliwość analizy jego zawartości; współdzielony analizator również odrzuca jednostkę PL/SQL, której nagłówek zawiera znacznik zabezpieczenia, zapobiegając błędnej klasyfikacji, która mogłaby generować wiarygodne, ale fałszywe dane statystyczne.

Zobacz [Inwentarz artefaktów innych niż tabele](ARTIFACT_INVENTORY.md), aby
poznać instrukcje operacyjne i pokrycie silników.

## Ochrona przed steganografią według wektora

| Wektor | Zabezpieczenie |
|---|---|
| Kolejność identyfikatorów. | DHMC-SHA256 z kluczem lokalnym dla procesu, oddzielony domenami, zapobiega sprawdzaniu nazw kandydatów w trybie offline. Ponowne użycie klucza jest dopuszczalne tylko wtedy, gdy wymagane są stabilne etykiety między kolejnymi uruchomieniami. |
| Najmniej znaczące bity liczb | Statystyki są domyślnie zaokrąglane do udokumentowanej dokładności. Tryb dokładnych długości jest jawny, wymaga zgody, jest rejestrowany w dzienniku audytu i musi być traktowany jako bardziej wrażliwe metadane. |
| Znacznik czasu poniżej sekundy | Jeden znacznik czasu UTC na początku, tylko z dokładnością do sekund |
| Formatowanie TOML | Standardowy wynik używa kolejności kluczy i wcięć, które się nie zmieniają. Zawiera tylko standardowy nagłówek i komentarze od producenta, bez komentarzy pochodzących z danych wejściowych. |
| Losowość pobierania próbek. | Pobieranie próbek wykorzystuje stałe wartości początkowe (deterministyczne `TABLESAMPLE SYSTEM` w PostgreSQL). Oddzielnie, anonimizacja identyfikatorów celowo pobiera klucz szyfrujący z generatora liczb losowych CSPRNG systemu operacyjnego, chyba że dostarczysz własny. |
|Niewykorzystane pola|Każde pole jest udokumentowane powyżej; nie ma pól "metadanych"/"komentarzy"/"rezerwowanych" zawierających nieograniczone dane|
| Tekst źródłowy artefaktów i materiały zewnętrzne | Definicje są tymczasowe i zerowane po ograniczonej analizie; nazwy, tekst SQL, punkty końcowe, nazwy dostawców, poświadczenia, klucze, certyfikaty, nazwy pakietów i pliki binarne nie mają serializowanego pola |

## Zgodność wersji schematu

Obecne wersje generują schemat wersji 7. Wersje od 1 do 6 są nadal akceptowane ze względu na kompatybilność wsteczną. Plik v1/v2 nie zawiera bloków dystrybucji. Plik w wersji 3 zawiera metadane dystrybucji, ale nie zawiera inwentarza artefaktów. Plik w wersji 4 może zawierać inwentarz artefaktów, ale jest starszy niż aktualne identyfikatory kontraktu Blueprint. Czytniki normalizują starsze identyfikatory w wersji 4 podczas odczytu i ponownie emitują ten dokument z kanonicznymi identyfikatorami Blueprint. Plik w wersji 5 jest starszy niż dowody dotyczące topologii i zakresu zestawów danych, dodane w wersji 6. Wersja 6 używa kontraktu topologii w wersji 1, kontraktu artefaktów w wersji 1, połączonych pól tabeli kind/partition, skal bez znaku, oraz opcjonalnej świeżości statystyk. Wersja 7 używa kontraktu topologii w wersji 2 i kontraktu artefaktów w wersji 2, wymaga wyraźnych dowodów structure/environment/statistics, oddziela ortogonalną semantykę tabel, obsługuje skal z znakiem i modele numeryczne Oracle, oraz wymaga wyraźnego stanu inwentarza artefaktów. Rezerwuje również stały kontrakt agregacji `dbwarp-blueprint-artifact-complexity/v1` bez dodawania pola oceny dla każdego obiektu lub pola do krzyżowej tabelaryzacji. Czytniki odrzucają nieznane, przyszłe wersje schematu z jasnym komunikatem o aktualizacji, zamiast cicho pomijać pola. Czytniki stosują taką samą ścisłą walidację w wersji 7 do samodzielnych i osadzonych Blueprintów, zamiast przepisywać nieprawidłowe dowody podczas parsowania.

## Dlaczego TOML, a nie JSON

- TOML czytelniej oddziela sekcje strukturalne od danych liści
  (`[tables.table-001.cols.col-2]` zamiast zagnieżdżonego JSON).
- Jest łatwiejszy do porównywania (jeden klucz na wiersz; podtabele oparte na
  identyfikatorach pozostają ciągłe).
- Przed udostępnieniem, zapoznaj się z polityką klasyfikacji danych obowiązującą w Twojej organizacji.

JSON jest używany jako **format pośredni** w zapasowej ścieżce SQL. Każdy skrypt
`sql/blueprint.*.sql` tworzy JSON, a `blueprint_format.py` normalizuje go do TOML.
Pośredni JSON zawiera rzeczywiste identyfikatory źródłowe; MySQL może także
zawierać deklaracje enum/set przez `COLUMN_TYPE`, dlatego plik musi pozostać
chroniony w środowisku źródłowym. Normalizator używa domyślnie nowego tajnego
klucza i akceptuje ten sam chroniony kontrakt `--anonymization-key-file` dla
zatwierdzonych porównań między uruchomieniami. Końcowym plikiem sprawdzonym do
udostępnienia DBWarp jest zawsze TOML.

## Rozszerzenia pochodzenia plików strukturalnych

Kiedy `engine` lub `source_kind` jest `"parquet"` lub `"avro"`, wersja schematu 3 lub nowsza może również generować następujące pola z ograniczoną ilością danych. Czytelnicy muszą zachować rozróżnienie między przechowywaniem danych w pliku źródłowym a pomiarami próbek z ograniczoną ilością danych; czytelnicy, którzy nie obsługują wersji schematu dokumentu, muszą odrzucić go z komunikatem o konieczności aktualizacji, zamiast odrzucać nieznane pola.

Wersje V7 struktur plików wymagają kompletnych bloków `[structure_scope]` i `[statistics_evidence]`, pomijają bloki `[database_topology]`, `[source_environment]` i `[activity_snapshot]`, oraz generują wyraźny wskaźnik "nie dotyczy" `[artifact_inventory]`. Nigdy nie wnioskują one o topologii bazy danych ani o wydajności serwera na podstawie hosta, na którym działa program do zbierania danych.

Blueprinty plików strukturalnych używają tych samych zanonimizowanych
identyfikatorów co Blueprinty baz danych: `table-NNN` w kolejności opartej na tajnym kluczu
oraz `col-N` według numeru porządkowego w schemacie. Nazwy plików,
ścieżki Parquet, nazwy pól Avro i wartość `logical_table` z manifestu nie są
emitowane jako identyfikatory tabel ani kolumn.

W zakresie tabeli, `table_bytes` to szacowany rozmiar transferu, natomiast `storage_bytes` to rzeczywisty rozmiar obiektu źródłowego na dysku. Parquet z metadanymi używa niekompresowanych bajtów kolumn dla `table_bytes`; opcjonalne dekodowane próbkowanie zastępuje ten szacunek prognozowanym rozmiarem `blueprint-compression-probe-v2` bajtów. Avro wylicza go na podstawie pełnego skanowania po dekodowaniu. Opcjonalne pola `source_partitions`, `row_group_count` i `source_codec` opisują układ pliku. Zbiory danych składające się z wielu plików agregują te wartości. `row_group_count` jest specyficzne dla Parquet; `source_partitions` to `1` dla pojedynczego obiektu wejściowego.

Na poziomie kolumny `null_fraction` jest obserwowaną wartością od `0.0` do
`1.0`. `length_sample_rows` i `length_sample_method` opisują sposób uzyskania
`len_avg` i `len_p95`. `source_semantics` przechowuje ograniczone fakty dotyczące
zgodności, takie jak `"repeated-leaf"`, `"nested-json"` lub
`"multi-type-union"`; nigdy nie zawiera nazw Twoich pól ani ich wartości.
Precyzja i skala liczb dziesiętnych, precyzja i semantyka UTC/lokalna znacznika czasu, UUID
oraz stały rozmiar binarny są przenoszone przez istniejące oczyszczone pola
skalarne i `native_type`.

Na poziomie tabeli `ratio_storage` porównuje `table_bytes` z rzeczywistymi
bajtami obiektu źródłowego. Dla kolumny Parquet porównuje nieskompresowane i
skompresowane bajty fragmentu kolumny ze stopki. Są to sygnały planowania
przechowywania plików, a nie estymacje zdekodowanych próbek. `ratio_zstd_3` i
`ratio_zstd_19` są porównywalne tylko wtedy, gdy `sample_encoding` ma wartość
`"blueprint-compression-probe-v2"`. Współczynników stopki Parquet ani kontenera
Avro nie wolno kopiować do tych pól zstd.

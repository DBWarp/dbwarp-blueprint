# Inwentarz artefaktów innych niż tabele

> **Uwaga dotycząca tłumaczenia:** to tłumaczenie wspomagane maszynowo wymaga jeszcze rodzimego przeglądu technicznego. Wiążąca jest [kanoniczna wersja angielska](../ARTIFACT_INVENTORY.md); ten tekst nie jest przeznaczony do zastosowań umownych.

**Języki:** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
[Français](../fr/ARTIFACT_INVENTORY.md) | [Español](../es/ARTIFACT_INVENTORY.md) |
**Polski** | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Pliki Blueprint mogą opisywać obiekty bazy danych, które nie są tabelami, oraz wymagania dotyczące wdrożenia, bez ujawniania nazw źródłowych, definicji, adresów końcowych, tajemnic, certyfikatów, kluczy ani plików binarnych. Ten katalog pomaga DBWarp oszacować złożoność migracji i zidentyfikować zadania, które wymagają pakietów, infrastruktury, zatwierdzenia bezpieczeństwa lub wsparcia podczas konwersji.

Inwentaryzacja nie jest deklaracją możliwości. Fakt, że dany obiekt jest raportowany, nie oznacza, że DBWarp może go automatycznie odtworzyć lub przetłumaczyć. Należy potwierdzić z DBWarp, jakie typy obiektów są obsługiwane.

## Poziomy szczegółowości

Opcja `--artifact-detail` wybiera kompromis między prywatnością a planowaniem:

| Wartość | Odczyty z bazy | Dane w pliku Blueprint | Zgoda |
|---|---|---|---|
| `none` | Bez katalogów inwentarza i definicji (sonda topologii zliczająca nadal działa) | Jawnie niezamówiony inwentarz v7; bez liczników i grafu | Bez dodatkowej zgody |
| `summary` | Katalogi artefaktów, bez definicji | Liczniki według rodzaju i klasy wymagań zewnętrznych | Domyślne; bez dodatkowej zgody |
| `graph` | Katalogi i metadane zależności, bez definicji | Liczniki, stabilne anonimowe rekordy i krawędzie | Wymaga `--yes` |
| `analyzed` | Katalogi, zależności i dostępne definicje | Graf oraz ograniczone pasma cech języka i złożoności | Wymaga `--yes` |

Domyślne jest `summary`. Użyj `none`, gdy polityka pozwala zebrać strukturę
tabel, lecz zabrania katalogów nietabelarycznych. `graph` służy do planowania
zależności bez odczytu definicji, a `analyzed` wymaga zatwierdzenia
tymczasowego odczytu definicji.

```bash
./dbwarp-blueprint \
  --connect postgresql://blueprint_user@db.internal/appdb \
  --password-file /etc/dbwarp/blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --artifact-detail analyzed \
  --out appdb.blueprint.toml \
  --audit-log appdb.blueprint.audit.txt \
  --yes
```

## Umowa prywatności

Wyjście artefaktów zawiera tylko ograniczone metadane z zamkniętego słownika:

- anonimowe identyfikatory spójne wewnątrz uruchomienia, np. `view-001`,
  `function-002` i `schema-A`; stabilność między uruchomieniami wymaga
  ponownego użycia tego samego chronionego pliku `--anonymization-key-file`;
- zamknięte tokeny rodzaju, podrodzaju, warstwy, widoczności i trybu bezpieczeństwa;
- typowane relacje wyłącznie przez anonimowe identyfikatory artefaktów lub tabel, z zamkniętymi tokenami dowodów i przyczyn nierozwiązania;
- ograniczone, kwalifikowane dla silnika wymagania funkcji do planowania migracji;
- liczniki i ograniczone pasma zamiast swobodnych opisów;
- standardowe etykiety katalogów, np. `pg_proc`, `information_schema.views` i `sys.objects`;
- klasy wymagań zewnętrznych, nigdy ich nazwy ani materiał.

Nie zawiera nazw obiektów źródłowych, tekstu SQL ani języka proceduralnego,
nazw schematów, podmiotów, adresów końcowych, dostawców, poświadczeń, kluczy,
treści certyfikatów, plików assembly, nazw pakietów rozszerzeń ani bibliotek
ładowalnych.

W trybie `analyzed` definicje pozostają w pamięci tylko na czas usunięcia
komentarzy i literałów oraz wyznaczenia ograniczonych agregatów leksykalnych.
Właściciel zeruje je przy zwolnieniu; nie są serializowane, logowane ani
wysyłane do usług. Jest to ograniczenie ekspozycji pamięci, a nie gwarancja
przeciw stronicowaniu systemu czy uprzywilejowanemu debuggerowi.

Anonimowy graf nadal może identyfikować aplikację przez liczby i topologię.
Dlatego `graph` i `analyzed` kończą się `DBP1014E` bez jawnego `--yes`.

## Dowody kompletności

Blok `[artifact_inventory]` jest celowo samokontrolujący:

| Pole | Znaczenie |
|---|---|
| `contract` | Niezależnie wersjonowana umowa; v7 używa `dbwarp-blueprint-artifacts/v2`, a starsze schematy Blueprint zachowują v1 |
| `detail` | Żądany poziom szczegółowości |
| `scope` | Zakres katalogu v7: `all-visible-schemas`, `selected-schemas`, `structured-source` albo `unknown` |
| `visibility` | `full`, `privilege_filtered` albo `unknown` |
| `inventory_complete` | Prawda tylko przy pełnej widoczności, bez nieczytelnych katalogów i zadeklarowanych niezamodelowanych rodzin |
| `dependencies_complete` | Prawda tylko wtedy, gdy źródła zależności były czytelne i zamodelowane rodziny są rozliczone |
| `requirements_complete` | Agregat V7: prawda tylko po sprawdzeniu wersji i edycji silnika, przy pełnym pokryciu populacji oceny w wybranym zakresie i `requirement_status = complete | not_applicable` dla każdego wyemitowanego artefaktu; pominięcie oznacza fałsz |
| `analysis_complete` | Prawda tylko dla `analyzed` i kompletnej analizy wszystkich dostępnych definicji |
| `catalogs_read` | Standardowe rodziny katalogów odczytane pomyślnie |
| `catalogs_unreadable` | Rodziny niedostępne lub zakończone błędem; odpowiednie deklaracje są obniżane bez kasowania niezależnych dowodów dla obiektów |
| `catalogs_not_applicable` | Rodziny dowiedzione jako nieodpowiednie; rozłączne ze zbiorami czytelnym i nieczytelnym |
| `families_not_inventoried` | Znane rodziny obiektów, których ta wersja nie uwzględnia w inwentarzu |

Błąd opcjonalnego katalogu nie usuwa obiektów po cichu. Program emituje
`DBP1410W`, zapisuje katalog i ustawia odpowiednie deklaracje kompletności na
fałsz. Konto o małych uprawnieniach może więc wygenerować użyteczny częściowy
inwentarz bez przedstawiania braku jako dowodu.

`object_count` liczy wyemitowane rekordy artefaktów, a nie wiersze pojedynczego
katalogu. Pakiety i typy obiektów Oracle są modelowane jako specyfikacje, ciała
i elementy; ciało posiada połączoną analizę języka. Obiekt widoczny w kilku
katalogach jest liczony raz. Metadane i źródło wyzwalacza są łączone według
natywnej tożsamości przed anonimizacją.

## Umowa zagregowanej złożoności

Schemat v7 definiuje wyłącznie zagregowany rekord
`[artifact_inventory.complexity]` dla `graph` i `analyzed`. Nie wymaga on
dodatkowego odczytu ani uprawnienia; ocena wynika z zatwierdzonego anonimowego
grafu i spisu języka. Jest wymagany dla `graph` i `analyzed`, a nie występuje dla
`none` i `summary`.

Raporty oceny obejmują siedem obszarów: objętość, przepływ sterowania, zakres funkcjonalności, splątanie zależności, powiązanie z otoczeniem, nieprzejrzystość i powiązanie z dialektami. Wyniki przedstawiane są w postaci zakresów, a nie wartości liczbowych. `overall_score` jest zarezerwowany i nie jest wypełniany, ponieważ wartość od 0 do 100 sugerowałaby niedostępną precyzję.

Każdy wymiar zawiera dokładny jednowymiarowy histogram kwalifikującej się
populacji. Objętość używa pasm rozmiaru, pozostałe sześć pasm liczności. Obie
postacie dodają koszyki `not_applicable` i `unknown` oraz spełniają
`eligible = assessed + not_applicable + unknown`. Nie ma oceny złożonej dla
obiektu ani tabel krzyżowych według rodzaju, funkcji czy schematu. Rozpoznane
obiekty generowane przez silnik i pomocnicze są wyłączone z oceny, lecz pozostają
w inwentarzu; obiekty tymczasowe i z brakującymi flagami nadal się kwalifikują.
Wtyczki zainstalowane lokalnie, assembly CLR, Java i biblioteki pozostają pracą migracyjną,
nawet gdy ich ciało jest nieczytelne; wyklucza je tylko jawna flaga silnika.

`assessment_population_complete` mówi, czy znane są wszystkie kwalifikujące się
obiekty. Jest niezależne od `inventory_complete`, a brak pola oznacza fałsz.
Niepełna populacja wymusza ogólne pasmo `unknown`, chyba że znana dolna granica
jest już `very-high`.

Pokrycie zapisuje się dla każdego wymiaru. `not_applicable` jest zakończoną
oceną. Ocena częściowa oznacza, że jeden odpowiedni wymiar jest znany, a inny
nie; brak oceny oznacza brak znanego odpowiedniego wymiaru. Nieznane dowody nie
oznaczają małej złożoności. Częściowy wymiar ma wartość `unknown`, chyba że jego
dolna granica jest już `very-high`; ogólne pasmo pojawia się tylko przy zgodnych
granicach. `graph` nie podaje ogólnego werdyktu dla niepustej populacji, bo nie
czyta definicji. Pełna pusta populacja to `not-applicable`.

Obiekty opakowane przyczyniają się do histogramu nieprzejrzystości w sekcji `unknown`, nawet jeśli nie można przeprowadzić pełnej analizy języka. Należy odczytać pasmo nieprzejrzystości wraz z jego pokryciem, aby małe, zaobserwowane pasmo nie było odczytywane bez znajomości jego pełnej populacji. Jeśli sama ocena się nie powiedzie, pełny spis jest zachowywany wraz z kanonicznym, zawierającym wszystkie nieznane wartości, zagregowanym wynikiem. Opakowane lub pominięte definicje, niekompletne grafy, niekompletne dowody spełnienia wymagań, ograniczone zakresy oraz nieobsługiwane języki lub dialekty pozostają wyraźnymi ograniczeniami. Ograniczenia są wyprowadzane z artefaktu i dowodów spisu, o ile to możliwe. `unsupported-dialect` pozostaje odrębnym pojęciem, ponieważ spis może nazwać dialekt i zgłosić `unavailable`, ale nie ma statusu `unsupported`; oznacza to, że definicja została odczytana, ale wskazany analizator nie obsługuje tego dialektu, a nie to, że źródło zostało pominięte lub opakowane.

Zapis zawiera pojedynczą wersję analizatora oraz posortowane zbiory zakresów analizy, dialektów i profili gramatycznych obecnych w dostępnym zbiorze danych. Dwa zapisy można porównać tylko wtedy, gdy te zbiory, umowa, oceniający, zakres i polityka dotycząca populacji są identyczne. Zbiór zachowuje złożoność dla każdego źródła i nigdy nie agreguje jej między silnikami lub analizatorami.

Wersja kontraktu i narzędzia do oceny złożoności są niezależne. Dokładne informacje o polach i niezmiennych parametrach można znaleźć w [Dokumentacji formatu](FORMAT.md).

## Pokrycie silników

Bieżący kolektor modeluje następujące rodziny:

| Silnik | Modelowane rodziny obiektów |
|---|---|
| PostgreSQL | widoki, widoki zmaterializowane, sekwencje, procedury i funkcje, agregaty, typy enum/domain/composite/range, wyzwalacze, wartości domyślne, ograniczenia check, polityki, reguły, wyzwalacze zdarzeń, rozszerzenia, obce tabele/serwery, publikacje, subskrypcje, przestrzenie tabel i funkcje natywne |
| MySQL | widoki, funkcje i procedury składowane, wyzwalacze, zdarzenia harmonogramu, zależności widoków, tabele FEDERATED i rejestracje ładowalnych UDF |
| SQL Server | widoki, procedury składowane, funkcje skalarne/tabelaryczne, moduły CLR, wyzwalacze, wartości domyślne, check, reguły, synonimy, sekwencje, typy użytkownika, assembly CLR, zewnętrzne obiekty danych, katalogi pełnotekstowe, obiekty partycjonowania, grupy plików inne niż PRIMARY, certyfikaty, klucze, poświadczenia bazodanowe, serwery połączone i zadania SQL Server Agent |

Każdy plik Blueprint wymienia znane niezamodelowane rodziny. Zerowy licznik nie
jest dowodem nieobecności, jeżeli `visibility`, pola kompletności i lista rodzin
nieobjętych inwentarzem tego nie potwierdzają.

## Dowody wymagań

Wymagania dotyczące artefaktów to informacje specyficzne dla silnika, pochodzące z ograniczonych kolumn katalogu lub dedykowanych kontroli składniowych, uwzględniających specyfikę silnika. Ogólna analiza leksykalna nie generuje wymagań specyficznych dla silnika. Jeśli zanalizowana funkcja języka odzwierciedla tę samą informację, wymaganie ma pierwszeństwo, a funkcja pozostaje jedynie obserwacją leksykalną. Brak wymagania nie jest dowodem na to, że każdy token wymagania został sprawdzony.

Każdy obiekt w wersji v7 graph/analyzed rejestruje `requirement_status` jako `complete`, `partial`, `unavailable` lub `not_applicable`. Tylko `complete` powoduje, że pusta lista jest dowodem braku wymagań dla tego obiektu. `partial` rejestruje, że pewien fakt lub producent zakończył się sukcesem bez pełnego pokrycia; `unavailable` rejestruje, że żaden producent nie ustanowił użytecznego pokrycia i dlatego nie może towarzyszyć znanemu wymaganiu lub dowodowi zewnętrznych zależności. Taki dowód wymaga `partial`. Oba elementy przyczyniają się do nieznanej złożoności wynikającej z wymagań, podczas gdy kompletne obiekty pozostają możliwe do oceny. `not_applicable` zabrania rejestrowania wymagań i zewnętrznych zależności. Wartość `requirements_complete` na poziomie inwentarza jest prawdziwa tylko po sprawdzeniu wersji i edycji silnika, pełnej ocenie i w przypadku, gdy każdy wyemitowany obiekt jest kompletny lub nie dotyczy. PostgreSQL, MySQL i SQL Server ustawiają tę wartość tylko wtedy, gdy każdy odpowiedni katalog artefaktów został sprawdzony, a populacja wybranej zakresu została uznana za kompletną. Odrzucony lub nieczytelny katalog, lub granica wyboru, której populacja nie może zostać udowodniona, utrzymuje tę wartość jako fałsz, nie usuwając kompletnych dowodów dla każdego obiektu z katalogów, które zostały odczytane. Wymagania dotyczące artefaktów nie są raportowane dla Oracle.

## Wymagania zewnętrzne

Obiekty zależne od czegoś więcej niż przenośne DDL tabeli otrzymują anonimową
klasę wymagania zewnętrznego:

| Klasa | Co musi rozstrzygnąć operator |
|---|---|
| `postgresql_extension` | Zgodny pakiet rozszerzenia i wersja celu |
| `postgresql_native_function` | Biblioteka natywna i zgodność ABI |
| `mysql_loadable_udf` | Ładowalny plik UDF i założenia ABI serwera źródłowego |
| `sqlserver_clr_assembly` | Włączenie CLR, assembly, środowisko wykonawcze i polityka zaufania |
| `foreign_endpoint` | Sieć, dostawca, zdalna baza i uwierzytelnianie |
| `replication_topology` | Topologia publikacji/subskrypcji i polityka celu |
| `physical_storage` | Projekt grup plików lub rozmieszczenia fizycznego |
| `server_feature` | Dostępność funkcji serwera lub usługi zarządzanej |
| `certificate_material` | Wydanie lub import certyfikatu zgodnie z polityką celu |
| `encryption_or_credential_material` | Klucze, poświadczenia, zewnętrzny magazyn i obsługa sekretów |
| `sqlserver_agent` | Dostępność agenta, środowisko i nadzór zadań |

Plik Blueprint wskazuje, czy potrzebny jest materiał binarny, tajny lub
dotyczący punktu końcowego, ale go nie przechwytuje. Obiekty zewnętrzne mają stać się jawnymi zadaniami
migracji, a nie pominięciami best-effort.

## Spis cech języka

`analyzed` Szczegóły dodają `dbwarp-language-feature-census/v1` bloków. Schemat v7 generuje `lexical-v2`, który analizuje tylko wykonywalną lub deklaratywną część i rejestruje `analysis_span = "executable-body"`. Wyklucza zewnętrzny wrapper tworzenia, identyfikator, sygnaturę, deklarację zwracaną oraz opcje modułu. Jeśli część nie może być bezpiecznie wyizolowana, kolektor rejestruje nieznany zakres i niedostępne dowody; obiekty bez zdefiniowanego wymiaru używają `not-applicable`. Pominięty zakres jest interpretowany jako nieznany, a nie wnioskowany z silnika. Analizator raportuje `status = "partial"` dla obsługiwanych definicji, ponieważ nie jest parserem, kompilatorem, semantycznym wiązaczem ani gwarancją powodzenia tłumaczenia. Brakujące lub nieobsługiwane dowody definicji to `unavailable`, a udowodniona niemożliwość analizy to `not_applicable`.

Rejestruje ograniczone pasma rozmiaru, liczby instrukcji i tokenów, zagnieżdżeń,
złożoności cyklomatycznej oraz obszarów nieprzezroczystych/dynamicznych.
Zamknięty słownik obejmuje sterowanie, złączenia, podzapytania, CTE, agregaty,
okna, DML, DDL, obiekty tymczasowe, dynamiczny SQL, JSON, XML, dane przestrzenne,
wektory, zgłaszane błędy, sterowanie transakcjami, ref cursors, typy zakotwiczone,
interwały, strefy czasowe, Boolean, LOB i bezpieczeństwo. Kontekst obejmuje profil gramatyki, tryby SQL MySQL
oraz dla SQL Server zgodność, `ANSI_NULLS` i `QUOTED_IDENTIFIER`.

Analizator leksykalny usuwa komentarze, literały w cudzysłowach oraz identyfikatory w cudzysłowach przed rozpoczęciem liczenia. Posiada zasady kontekstowe dla deklaracji zdarzeń wyzwalaczy, PostgreSQL `EXECUTE FUNCTION` oraz opcji modułów SQL Server. Mimo to wszystkie wyniki pozostają jedynie przybliżonymi danymi planistycznymi. Opakowany kod PL/SQL jest odrzucany; zaciemnione bajty nigdy nie stają się wiarygodnymi pomiarami treści programu.

## Zalecany przebieg przeglądu

1. Uruchom domyślny poziom `summary` wraz z przeglądem katalogów artefaktów.
   Jeśli polityka pozwala tylko na katalogi tabel, użyj zamiast tego
   `--artifact-detail none`; v7 jawnie zapisuje tę decyzję zamiast pomijać stan
   inwentarza.
2. Sprawdź liczniki, klasy zewnętrzne, widoczność, nieczytelne katalogi i niezamodelowane rodziny.
3. Zatwierdź `graph` tylko, gdy anonimowa topologia jest akceptowalna.
4. Zatwierdź `analyzed` tylko, gdy akceptujesz tymczasowy odczyt definicji.
5. Zachowaj dziennik audytu lokalnie jako dowód z kontrolą dostępu. Udostępniaj
   go tylko wtedy, gdy wskazany odbiorca potrzebuje szczegółów dotyczących
   punktu końcowego, tożsamości, ścieżki i degradacji, za pośrednictwem
   zatwierdzonego bezpiecznego kanału.
6. Nie zakładaj, że zinwentaryzowany obiekt może zostać automatycznie odtworzony lub przetłumaczony; potwierdź to z DBWarp.

Dokładne pola serializowane opisuje [dokumentacja formatu](FORMAT.md). Odczyty, zapisy, ostrzeżenia i deklaracje zaufania w czasie działania opisuje [dokumentacja audytu](AUDIT.md).

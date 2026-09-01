# Inwentarz artefaktów innych niż tabele

> **Uwaga dotycząca tłumaczenia:** to tłumaczenie wspomagane maszynowo wymaga jeszcze rodzimego przeglądu technicznego. Wiążąca jest [kanoniczna wersja angielska](../ARTIFACT_INVENTORY.md); ten tekst nie jest przeznaczony do zastosowań umownych.

**Języki:** [English](../ARTIFACT_INVENTORY.md) | [Deutsch](../de/ARTIFACT_INVENTORY.md) |
[Français](../fr/ARTIFACT_INVENTORY.md) | [Español](../es/ARTIFACT_INVENTORY.md) |
**Polski** | [日本語](../ja/ARTIFACT_INVENTORY.md) |
[简体中文](../zh/ARTIFACT_INVENTORY.md)

Od schematu v4 Blueprinty opisują obiekty bazodanowe inne niż tabele oraz wymagania wdrożeniowe
bez publikowania nazw źródłowych, definicji, adresów końcowych, sekretów,
certyfikatów, kluczy ani plików binarnych. Inwentarz pomaga DBWarp oszacować
złożoność migracji i wskazać zadania wymagające pakietów, infrastruktury,
zgody bezpieczeństwa lub konwersji wspomaganej.

Inwentarz nie jest deklaracją możliwości. Zarejestrowanie obiektu nie oznacza,
że DBWarp potrafi automatycznie go odtworzyć lub przetłumaczyć. Możliwości
migracji trzeba osobno sprawdzić w macierzy tras i artefaktów DBWarp.

## Poziomy szczegółowości

Opcja `--artifact-detail` wybiera kompromis między prywatnością a planowaniem:

| Wartość | Odczyty z bazy | Dane w pliku Blueprint | Zgoda |
|---|---|---|---|
| `none` | Bez katalogów i definicji artefaktów | Bez liczników i grafu | Bez dodatkowej zgody |
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

- stabilne anonimowe identyfikatory, np. `view-001`, `function-002` i `schema-A`;
- zamknięte tokeny rodzaju, podrodzaju, warstwy, widoczności i trybu bezpieczeństwa;
- zależności wyłącznie przez anonimowe identyfikatory artefaktów lub tabel;
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
| `contract` | Niezależnie wersjonowana umowa, obecnie `dbwarp-blueprint-artifacts/v1` |
| `detail` | Żądany poziom szczegółowości |
| `visibility` | `full`, `privilege_filtered` albo `unknown` |
| `inventory_complete` | Prawda tylko przy pełnej widoczności, bez nieczytelnych katalogów i zadeklarowanych niezamodelowanych rodzin |
| `dependencies_complete` | Prawda tylko wtedy, gdy źródła zależności były czytelne i zamodelowane rodziny są rozliczone |
| `analysis_complete` | Prawda tylko dla `analyzed` i kompletnej analizy wszystkich dostępnych definicji |
| `catalogs_read` | Standardowe rodziny katalogów odczytane pomyślnie |
| `catalogs_unreadable` | Rodziny niedostępne lub zakończone błędem |
| `families_not_inventoried` | Znane rodziny poza bieżącą umową kolektora |

Błąd opcjonalnego katalogu nie usuwa obiektów po cichu. Program emituje
`DBP1410W`, zapisuje katalog i ustawia odpowiednie deklaracje kompletności na
fałsz. Konto o małych uprawnieniach może więc wygenerować użyteczny częściowy
inwentarz bez przedstawiania braku jako dowodu.

## Pokrycie silników

Kolektor v1 modeluje następujące rodziny:

| Silnik | Modelowane rodziny obiektów |
|---|---|
| PostgreSQL | widoki, widoki zmaterializowane, sekwencje, procedury i funkcje, agregaty, typy enum/domain/composite/range, wyzwalacze, wartości domyślne, ograniczenia check, polityki, reguły, wyzwalacze zdarzeń, rozszerzenia, obce tabele/serwery, publikacje, subskrypcje, przestrzenie tabel i funkcje natywne |
| MySQL | widoki, funkcje i procedury składowane, wyzwalacze, zdarzenia harmonogramu, zależności widoków, tabele FEDERATED i rejestracje ładowalnych UDF |
| SQL Server | widoki, procedury składowane, funkcje skalarne/tabelaryczne, moduły CLR, wyzwalacze, wartości domyślne, check, reguły, synonimy, sekwencje, typy użytkownika, assembly CLR, zewnętrzne obiekty danych, katalogi pełnotekstowe, obiekty partycjonowania, grupy plików inne niż PRIMARY, certyfikaty, klucze, poświadczenia bazodanowe, serwery połączone i zadania SQL Server Agent |

Każdy plik Blueprint wymienia znane niezamodelowane rodziny. Zerowy licznik nie
jest dowodem nieobecności, jeżeli `visibility`, pola kompletności i lista rodzin
nieobjętych inwentarzem tego nie potwierdzają.

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

Plik Blueprint wskazuje, czy potrzebny jest materiał binarny, tajny lub końcowy,
ale go nie przechwytuje. Obiekty zewnętrzne mają stać się jawnymi zadaniami
migracji, a nie pominięciami best-effort.

## Spis cech języka

Poziom `analyzed` dodaje bloki `dbwarp-language-feature-census/v1` dla
dostępnych definicji SQL i proceduralnych. Pierwszy analizator to `lexical-v1`
ze `status = "partial"`; nie jest parserem, kompilatorem, binderem semantycznym
ani gwarancją powodzenia tłumaczenia.

Rejestruje ograniczone pasma rozmiaru, liczby instrukcji i tokenów, zagnieżdżeń,
złożoności cyklomatycznej oraz obszarów nieprzezroczystych/dynamicznych.
Zamknięty słownik obejmuje sterowanie, złączenia, podzapytania, CTE, agregaty,
okna, DML, DDL, obiekty tymczasowe, dynamiczny SQL, JSON, XML, dane przestrzenne,
wektory i bezpieczeństwo. Kontekst obejmuje profil gramatyki, tryby SQL MySQL
oraz dla SQL Server zgodność, `ANSI_NULLS` i `QUOTED_IDENTIFIER`.

Analizator usuwa komentarze, literały i cytowane identyfikatory. Reguły
kontekstowe obsługują zdarzenia wyzwalaczy, PostgreSQL `EXECUTE FUNCTION` i
opcje modułów SQL Server. Wyniki pozostają zgrubnym dowodem planistycznym.
Przyszły analizator gramatyczny może zmienić wersję bez zmiany umowy zewnętrznej.

## Zalecany przebieg przeglądu

1. Uruchom `summary` razem ze zwykłym przeglądem katalogów.
2. Sprawdź liczniki, klasy zewnętrzne, widoczność, nieczytelne katalogi i niezamodelowane rodziny.
3. Zatwierdź `graph` tylko, gdy anonimowa topologia jest akceptowalna.
4. Zatwierdź `analyzed` tylko, gdy akceptujesz tymczasowy odczyt definicji.
5. Zachowaj dziennik audytu lokalnie jako dowód z kontrolą dostępu. Udostępniaj
   go tylko wtedy, gdy wskazany odbiorca potrzebuje szczegółów dotyczących
   punktu końcowego, tożsamości, ścieżki i degradacji, za pośrednictwem
   zatwierdzonego bezpiecznego kanału.
6. Porównaj inwentarz z macierzą możliwości DBWarp przed obietnicą automatycznego odtworzenia lub tłumaczenia.

Dokładne pola opisuje [Dokumentacja formatu](FORMAT.md). Odczyty, zapisy,
ostrzeżenia i deklaracje zaufania opisuje [Dokumentacja audytu](AUDIT.md).

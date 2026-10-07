# Historia zmian

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i może zawierać błędy. Nie powinna być traktowana jako tekst kontraktowy. Zobacz [kanoniczne źródło angielskie](../../CHANGELOG.md).

**Języki:** [English](../../CHANGELOG.md) | [Deutsch](../de/CHANGELOG.md) | [Français](../fr/CHANGELOG.md) | [Español](../es/CHANGELOG.md) | **Polski** | [日本語](../ja/CHANGELOG.md) | [简体中文](../zh/CHANGELOG.md)

Wersje wydań identyfikują kolektor. Wersja schematu Blueprint i kodowanie próbek
kompresji są odrębnymi kontraktami zgodności; zobacz [FORMAT.md](FORMAT.md) i
[pomiar kompresji](COMPRESSION_MEASUREMENT.md).

## 1.6.0

### Schemat Blueprint v7

- Zapisuje schemat v7 z jawną klasyfikacją tabel, organizacją pamięci,
  partycjonowaniem, stanem segmentów, kolejnością kolumn oraz bogatszą
  semantyką typów, wartości NULL, wartości generowanych i tożsamości.
- Dodaje dowody dla każdej tabeli i całego przechwycenia dotyczące struktury,
  liczby wierszy, przydzielonych bajtów, statystyk i obserwacji środowiska
  źródłowego. Brakujące, niedostępne, nieprawidłowe lub ograniczone wyborem
  dowody pozostają jawne, zamiast być przedstawiane jako zmierzone zero lub
  kompletny inwentarz.
- Dodaje do inwentarza artefaktów oparty na katalogu stan wymagań dla każdego
  obiektu i ograniczone dowody złożoności. Łączna kompletność pozostaje
  fałszywa, jeśli nie można dowieść kompletności wymaganego katalogu lub
  wybranej populacji oceny.
- Zachowaj kompatybilność z formatami schematu od schema-v1 do schema-v6. Wcześniejsze wersje mogą nie odczytywać plików w formacie schema-v7.
### Wierność i bezpieczeństwo przechwytywania

- Rozróżnia pełne ograniczone odczyty, częściowe próbki i oszacowania katalogu
  w PostgreSQL, MySQL i SQL Server, w tym granice zabezpieczeń wierszy i
  dziedziczenia, adaptacyjne okna zakresów MySQL oraz tabele SQL Server
  zoptymalizowane pod kątem pamięci.
- Zaostrza spójność kardynalności, liczby wartości NULL, partycji, relacji i
  agregatów, aby zaokrąglone lub niepełne dowody nie stały się dokładnym
  twierdzeniem.
- Raportuje zakres obsługi zewnętrznych tabel SQL Server jako jawne
  ograniczenie, gdy PolyBase nie jest zainstalowany.
- Utrzymuje przechwytywanie w granicach liczby wierszy, bajtów, wartości i
  czasu. Degradacja niekrytyczna pozostaje widoczna w Blueprint i audycie ze
  stabilnymi kodami komunikatów.

### Granica zakresu Oracle

- W tej wersji dodano możliwość pobierania metadanych katalogu Oracle Basic dla
  Oracle 12c, 19c, 21c i 23ai/26ai jako podgląd, który wymaga wyraźnego
  potwierdzenia, dotyczy tylko katalogu i nie odczytuje żadnych wierszy z tabel.
  Szczegóły dotyczące
  ograniczeń znajdują się w `sql/grants/ORACLE_PREVIEW.md`.

### Artefakty wydania i uwierzytelnianie

- Archiwa wydania dla systemu Linux zawierają uwierzytelnianie Kerberos/GSSAPI
  dla SQL Server. Ładują środowisko uruchomieniowe Kerberos platformy tylko po
  wybraniu uwierzytelniania zintegrowanego, dzięki czemu kolektor uruchamia się
  bez bibliotek Kerberos; brak środowiska uruchomieniowego jest zgłaszany jako
  `DBP1604E` tylko dla uwierzytelniania zintegrowanego. Binaria wydania dla
  systemu Windows nadal zawierają uwierzytelnianie SSPI dla SQL Server.
- Oba tryby zintegrowane używają poświadczeń systemu operacyjnego i odmawiają
  połączenia, gdy podmiot serwera nie jest zgodny z oczekiwanym.

### Obsługa i zgodność

- Skrypty uprawnień Enhanced dla SQL Server dodają w oddzielnej partii jedno
  uprawnienie na poziomie serwera: `VIEW SERVER STATE` dla SQL Server 2019 oraz
  `VIEW SERVER PERFORMANCE STATE` dla wersji 2022 i 2025. Dzięki temu
  przechwytywanie Enhanced z opcją `--artifact-detail graph` lub `analyzed`
  może raportować przybliżone zakresy CPU i pamięci dla serwera zarządzanego
  samodzielnie. Poziomy Basic i Standard nie przyznają tego uprawnienia i
  raportują te zakresy jako nieznane; DBA może usunąć tę partię, aby zachować
  Enhanced bez tego uprawnienia.
- Aktualizuje stos zależności SQL Server, PostgreSQL, Parquet i uwierzytelniania
  Windows do wersji, które usuwają opublikowane problemy z bezpieczeństwem.
  Sterownik SQL Server przechodzi na wersję 0.13; `--tls-ca` zachowuje swoje
  restrykcyjne znaczenie i ufa tylko dostarczonemu urzędowi certyfikacji.
  `--max-wall-secs` pozostaje jedynym limitem czasu dla całego przechwytywania.
- SQL Server odczytuje pojemność systemu operacyjnego tylko wtedy, gdy żądana
  jest analiza obiektów innych niż tabele (`--artifact-detail graph` lub
  `analyzed`). Pozostałe przechwytywania zapisują zakresy pojemności jako
  niezażądane zamiast jako nieudany odczyt.
- Dokumentacja angielska pozostaje wiążąca. Przetłumaczony Markdown jest
dodatkowe informacje i zawiera własną informację o tłumaczeniu.

### Prezentacja

- Dodaje do prezentacji slajd dotyczący obiektów innych niż tabele oraz osobny
  slajd dotyczący złożoności artefaktów. Slajd złożoności pojawia się tylko po
  przechwyceniu złożoności i pokazuje pokrycie obok każdego poziomu, dzięki
  czemu niepełne dowody nigdy nie są przedstawiane jako niska złożoność.

## 1.5.1

### Przechwytywanie i wierność

- Zachowuje schemat Blueprint v6 i rozróżnia oszacowania katalogowe, obserwacje
  próbek oraz niedostępne dowody świeżości statystyk.
- Ulepsza pomiar ładunków binarnych, profilowanie już skompresowanych ładunków i
ograniczone testy kompresji. Wartości z różnych ustawień `sample_encoding` nie są ze sobą wymienne; sprawdź kodowanie przed porównywaniem współczynników.
- Ogranicza adaptacyjne ponowienia próbkowania MySQL i SQL Server, w tym
  nadmiernie duże wartości oraz zwiększenie rozmiaru wskutek zestawu znaków.
  Zapisuje pozostałe obciążenie wynikające z prefiksów, zachowując metadane
  pierwotnych długości próbkowanych wartości.
- Poprawia wykrywanie obcięcia w MySQL, gdy zestaw znaków połączenia zmienia
  długość zwracanych danych w bajtach.

### Obsługa i przegląd

- Doprecyzowuje konfigurację dedykowanych kont z minimalnymi uprawnieniami,
weryfikacja porównania i obsługiwane wersje baz danych.
- Odświeża dokumentację tłumaczoną maszynowo oraz komunikaty podczas działania,
  zachowując angielski jako wersję wiążącą, a tłumaczenia jako uzupełnienie.
- Dodaje historię wydań oraz wskazówki dotyczące wsparcia i współtworzenia do
  dystrybucji źródeł i plików binarnych.
### Kompatybilność.

Istniejące pliki Blueprint pozostają czytelne dla nowego kolektora. Odwrotna sytuacja nie jest gwarantowana: czytnik wersji 1.5.0 odrzuca nowe, opcjonalne pole `sample_layout`, a starsze wersje mogą odrzucać nowe kodowania próbek kompresji. Używaj tej samej wersji do tworzenia pliku i do generowania prezentacji na jego podstawie.

Zabezpiecz konkretną wersję oprogramowania i sumę kontrolną. Wyniki z jednej wersji nie muszą być takie same jak wyniki z innej.

## 1.5.0

Poprzednie wydanie zapewnia przechwytywanie schematu v6 dla PostgreSQL, MySQL i
SQL Server, inspekcję plików strukturalnych, lokalny zapis Blueprint i
prezentacji oraz skrypty nadawania uprawnień uwzględniające wersję. Dokładne
źródła i artefakty znajdują się przy
[tagu wydania](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0).

Informacje o zgłaszaniu problemów zawiera [SUPPORT.md](SUPPORT.md), a wskazówki
dotyczące współtworzenia — [CONTRIBUTING.md](CONTRIBUTING.md).

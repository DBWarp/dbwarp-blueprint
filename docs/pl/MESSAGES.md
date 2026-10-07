# Kody komunikatów operatorskich

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../MESSAGES.md).

[English](../MESSAGES.md) | [Deutsch](../de/MESSAGES.md) | [Français](../fr/MESSAGES.md) | [Español](../es/MESSAGES.md) | [Polski](MESSAGES.md) | [日本語](../ja/MESSAGES.md) | [简体中文](../zh/MESSAGES.md)

`dbwarp-blueprint` wykorzystuje stabilne identyfikatory komunikatów dla błędów walidacji i przepływu pracy związanych z DBWarp. Każdy komunikat zawiera prefiks podsystemu, identyfikator numeryczny oraz wskaźnik ważności, opisuje problem i proponuje działanie naprawcze.

## Format

```text
DBPnnnnS message text. Next: corrective action.
```

Pola:

- `DBP` oznacza DBWarp Blueprint.
- `nnnn` jest stabilnym czterocyfrowym numerem komunikatu.
- `S` oznacza ważność: `E` błąd, `W` ostrzeżenie, `I` informacja.

Kod jest stabilny i niezależny od języka. Jego podsumowanie, przyczyna i działanie naprawcze są lokalizowane, gdy `--lang` lub ustawienia językowe procesu wybiorą obsługiwany język. Dynamiczne informacje dotyczące systemu operacyjnego, sterowników baz danych, ścieżek i łańcucha przyczyn pozostają niezmienione, aby można było wyszukać oryginalny błąd. Tekst wiadomości nie może zawierać poufnych informacji ani niezaanonimizowanych adresów połączeń.

## Zakresy

| Zakres | Obszar |
|---|---|
| `DBP0001E` | Rzeczywiście niesklasyfikowana opakowana awaria z łańcuchem przyczyn |
| `DBP10xxE` | Walidacja polecenia, danych wejściowych połączenia i zasad zbierania |
| `DBP11xxE` | Walidacja manifestu wsadowego i danych wejściowych źródła |
| `DBP12xxE` | Selektory pakietów i selektory URI Blueprint |
| `DBP13xxE` | Walidacja TOML/prezentacji/schematu offline |
| `DBP14xxE/W` | Awarie przechwytywania bazy danych na żywo i niekrytyczne pogorszenie próbkowania |
| `DBP15xxE/W` | Plik strukturalny, Blueprint, prezentacja i dane wyjściowe audytu |
| `DBP16xxE/W` | Zasady poświadczeń, uwierzytelniania, TLS i plików poufnych |
| `DBP17xxE` | Zgoda operatora |
| `DBP18xxE` | Inicjalizacja środowiska uruchomieniowego procesu |

## Bieżące kody

| Kod | Znaczenie |
|---|---|
| `DBP0001E` | Niesklasyfikowana awaria; dalej znajduje się łańcuch przyczyn. |
| `DBP1000E` | Brak `--connect` poza trybami offline. |
| `DBP1001E` | Odrzucono hasło osadzone w URI. |
| `DBP1002E` | Nieobsługiwany schemat URI `--connect`. |
| `DBP1003E` | Nieobsługiwane nadpisanie nazwy serwera TLS. |
| `DBP1004E` | Flaga tokenu Azure użyta z silnikiem innym niż SQL Server. |
| `DBP1005E` | Tryb uwierzytelniania jest niedostępny dla wybranego silnika. |
| `DBP1006E` | Zażądano próbkowania kompresji plików strukturalnych bez jawnego `--yes`. |
| `DBP1007E` | Żądany tryb zachowania dokładnej długości dla silnika, który go nie obsługuje. |
| `DBP1008E` | `--preserve-exact-lengths` powoduje konflikt z wymogiem ścisłego zachowania długości. |
| `DBP1009E` | Zażądano dokładnej wierności długości próbek bez jawnego `--yes`. |
| `DBP1010E` | Wbudowany katalog lokalizacji jest niekompletny lub niespójny. |
| `DBP1011E` | Argumenty wiersza poleceń są nieprawidłowe. |
| `DBP1012E` | Obsługiwany URI połączenia z bazą danych ma nieprawidłową składnię. |
| `DBP1013E` | `--source-kind` jest puste lub nieobsługiwane. |
| `DBP1014E` | Zażądano anonimowego grafu artefaktów lub analizy definicji bez wyraźnej zgody. |
| `DBP1015E` | Opcje certyfikatu klienta TLS użyte z SQL Server, którego sterownik ich nie implementuje. |
| `DBP1101E` | Nie można odczytać manifestu wsadowego. |
| `DBP1102E` | Nie można przeanalizować manifestu wsadowego. |
| `DBP1103E` | Manifest wsadowy nie zawiera wpisów `[[source]]`. |
| `DBP1104E` | Tryb wsadowy wymaga jawnego `--yes`. |
| `DBP1105E` | Jedno źródło wewnątrz zadania wsadowego zakończyło się niepowodzeniem. |
| `DBP1106E` | Nieobsługiwany rodzaj źródła wsadowego. |
| `DBP1107E` | Dla źródła plikowego nie znaleziono plików wejściowych. |
| `DBP1108E` | Nieobsługiwany tryb zestawu plików. |
| `DBP1109E` | Identyfikator źródła wsadowego nie zawiera użytecznej litery lub cyfry ASCII. |
| `DBP1110E` | Źródło bazy danych ma niewłaściwą liczbę źródeł połączenia. |
| `DBP1111E` | Brakuje zmiennej `connect_env` albo nie można jej odczytać. |
| `DBP1112E` | Brakuje pliku `connect_file` albo nie można go odczytać. |
| `DBP1113E` | Nie można ukończyć danych wyjściowych zadania wsadowego, audytu, raportu lub katalogu. |
| `DBP1114E` | Elementy zestawu plików strukturalnych są niezgodne. |
| `DBP1115E` | Wszystkie źródła wsadu zawiodły; opublikowano tylko wynik diagnostyczny. |
| `DBP1116E` | Opublikowano częściowy pakiet wsadu. |
| `DBP1200E` | Nieprawidłowy selektor lub składnia `blueprint://`. |
| `DBP1201E` | Selektor pakietu nie dopasował żadnego źródła. |
| `DBP1202E` | Selektor pakietu dopasował wiele źródeł. |
| `DBP1203E` | Selektor pakietu nie dopasował żadnego możliwego do wyodrębnienia Blueprint ani tabeli. |
| `DBP1204E` | Nie można odczytać danych wejściowych pakietu. |
| `DBP1205E` | Zawartość pakietu lub wskazanego Blueprint jest nieprawidłowa. |
| `DBP1206E` | Nie można zapisać danych wyjściowych pakietu. |
| `DBP1301E` | Dla `--from-toml` brakuje `--deck`. |
| `DBP1302E` | Nieobsługiwana wersja schematu TOML Blueprint. |
| `DBP1401E` | Awaria na granicy przechwytywania PostgreSQL. |
| `DBP1402E` | Awaria na granicy przechwytywania MySQL. |
| `DBP1403E` | Awaria na granicy przechwytywania SQL Server. |
| `DBP1404W` | Tryb PostgreSQL TLS `prefer` przeszedł na tekst jawny dla pętli zwrotnej. |
| `DBP1405W` | Opcjonalny pomiar RTT bazy danych był niedostępny. |
| `DBP1406W` | Wyczerpano budżet czasu próbkowania poziomu 2. |
| `DBP1407W` | Próbka kompresji była niekompletna lub niedostępna; mogły zostać zachowane użyteczne wiersze z częściowej próbki. |
| `DBP1408W` | Próbka stylu kolumny tekstowej była niedostępna. |
| `DBP1409W` | Asynchroniczne zadanie połączenia PostgreSQL zgłosiło błąd. |
| `DBP1410W` | Opcjonalny katalog artefaktów był niedostępny, dlatego kompletność została jawnie obniżona. |
| `DBP1411W` | Dowód topologii jest niedostępny; wdrożenie i rola lokalna pozostają nieznane. |
| `DBP1412W` | Wykryto układ rozproszony lub shardowany, ale pełne wymiarowanie zbiorcze było niedostępne. |
| `DBP1413W` | Pokrycie tabel, wierszy lub bajtów jest niepełne albo nieznane. |
| `DBP1414W` | Relacja źródła pakietu jest nieznana, więc obliczenia między źródłami są niebezpieczne. |
| `DBP1415W` | Zadeklarowane repliki różnią się; zachowano deterministycznego reprezentanta bez uśredniania. |
| `DBP1416W` | Grupa shardów jest niepełna i nie wnosi sum zbiorczych. |
| `DBP1417W` | Zbiorcze sumy pakietu zostały wyłączone. |
| `DBP1418W` | Źródło uwzględnione w obliczeniach pakietu ma niepełne lub nieznane pokrycie. |
| `DBP1419E` | Przechwytywanie na żywo przekroczyło `--max-wall-secs`; klient zerwał połączenie i zgłasza limit serwera właściwy dla silnika. |
| `DBP1420E` | Co najmniej jeden żądany `--schema` nie był widoczny, dlatego nie zapisano Blueprint o niejednoznacznym zakresie. |
| `DBP1421W` | Tożsamości sesji SQL Server były niedostępne; przechwytywanie kontynuowano bez potwierdzenia tożsamości. |
| `DBP1422W` | Ocena złożoności artefaktów nie powiodła się; inwentarz zachowano, a dotknięte wymiary agregatów są nieznane. |
| `DBP1423W` | Katalog struktury indeksów lub relacji był niedostępny; podstawowe tabele i kolumny zachowano z jawnym oznaczeniem niepełnego pokrycia. |
| `DBP1424W` | Nie można było potwierdzić pełnej widoczności katalogu zasad bezpieczeństwa SQL Server; próbkowanie Tier 2 pominięto dla każdej dotkniętej tabeli. |
| `DBP1425W` | SQL Server zgłosił aktywny filtr zabezpieczeń wierszy; próbkowanie Tier 2 celowo pominięto zamiast mierzyć przefiltrowany podzbiór. |
| `DBP1426E` | Podczas konfiguracji, uruchamiania SQL*Plus, rozwiązywania nazw właścicieli, przechwytywania lub mapowania wystąpił błąd podstawowego przechwytywania danych Oracle. |
| `DBP1427W` | Nie można było w pełni potwierdzić pochodzenia wersji klienta Oracle SQL*Plus; przechwytywanie kontynuowano z jawnym ograniczeniem. |
| `DBP1428W` | Oracle Basic zatrzymał się przed wykonaniem każdego z planowanych zapytań do katalogu; odczytane tabele, kolumny, wiersze i rozmiary zostały zachowane, a Blueprint został oznaczony jako niekompletny. |
| `DBP1429W` | Wersja Oracle Basic zachowała podstawowe dane dotyczące tabel, kolumn, wierszy i rozmiarów, gdy co najmniej jedno dodatkowe zapytanie katalogowe było niedostępne. |
| `DBP1430W` | Oracle Basic opublikowało swój Blueprint, ale nie mogło utworzyć opcjonalnego pliku strumienia offline. |
| `DBP1501E` | Awaria na granicy przechwytywania pliku strukturalnego. |
| `DBP1502E` | Niepowodzenie danych wyjściowych Blueprint lub pakietu. |
| `DBP1503E` | Niepowodzenie generowania prezentacji PowerPoint. |
| `DBP1504W` | Nie można zapisać dziennika audytu. |
| `DBP1505E` | Strumień offline Oracle Basic nie przeszedł walidacji dotyczącej uprawnień do plików, limitu rozmiaru, sum kontrolnych, zestawu zapytań lub struktury katalogu. |
| `DBP1601E` | Niepowodzenie uzyskania poświadczeń. |
| `DBP1602E` | Niepowodzenie konfiguracji TLS. |
| `DBP1603E` | Niepowodzenie uzyskania nazwy użytkownika bazy danych. |
| `DBP1604E` | Konfiguracja uwierzytelniania bazy danych jest nieprawidłowa. |
| `DBP1605W` | Egzekwowanie uprawnień do plików poufnych jest niedostępne na tej platformie. |
| `DBP1606E` | Asercja uwierzytelnionego podmiotu SQL Server nie powiodła się przed przechwyceniem katalogu. |
| `DBP1607E` | Nie można było bezpiecznie zainicjować klucza HMAC anonimizacji. |
| `DBP1701E` | Operację anulowano przed udzieleniem jawnej zgody. |
| `DBP1702E` | Nie można odczytać odpowiedzi na monit o zgodę ze standardowego wejścia. |
| `DBP1801E` | Nie można zainicjować asynchronicznego środowiska uruchomieniowego. |

Każdy obsługiwany język zawiera wszystkie podsumowania, przyczyny i działania związane z DBP. Program sprawdza to podczas uruchamiania i w przypadku błędu wyświetla komunikat `DBP1010E` zamiast cicho przełączać się na język angielski.

Niekrytyczne ostrzeżenia dotyczące próbkowania bazy danych są wypisywane ze
stabilnym kodem ostrzeżenia i zapisywane w audycie uruchomienia. Pozwala to
odróżnić pełne przechwycenie poziomu 2 od udanego, ale częściowo próbkowanego
przechwycenia, bez przekształcania awarii opcjonalnego pomiaru w całkowitą
awarię zbierania.

## Lista kontrolna dla pomocy technicznej

Przy zgłaszaniu awarii do pomocy technicznej podaj:

- kompletne dane wyjściowe terminala, w tym kod `DBP`;
- dziennik audytu, jeśli użyto `--audit-log`;
- wiersz polecenia z usuniętymi lub zamaskowanymi danymi wrażliwymi;
- w przypadku błędów pakietu dane wyjściowe `dbwarp-blueprint --bundle-list ...`.

Nie proś o pliki haseł, pliki tokenów, klucze prywatne ani surowe próbki wierszy bazy danych.

# Kody komunikatów operatorskich

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../MESSAGES.md).

[English](../MESSAGES.md) | [Deutsch](../de/MESSAGES.md) | [Français](../fr/MESSAGES.md) | [Español](../es/MESSAGES.md) | [Polski](MESSAGES.md) | [日本語](../ja/MESSAGES.md) | [简体中文](../zh/MESSAGES.md)

`dbwarp-blueprint` używa stabilnych identyfikatorów komunikatów operatorskich dla walidacji i awarii przepływu pracy będących pod kontrolą DBWarp.
Format jest inspirowany komunikatami operatorskimi w stylu IBM: prefiks podsystemu, identyfikator liczbowy i sufiks ważności.
Dokumentacja IBM CICS opisuje identyfikator programu, czterocyfrowy numer komunikatu i literę ważności; IBM MQ podobnie używa pól komponentu/prefiksu, identyfikatora liczbowego i końcowego kodu typu komunikatu. Wytyczne Microsoft dotyczące komunikatów błędów wzmacniają praktyczną zasadę, że błąd powinien opisywać problem i podawać działanie, które użytkownik może wykonać.

Materiały referencyjne:

- Format komunikatów IBM CICS: https://www.ibm.com/docs/en/cics-pa/5.3.0?topic=messages-message-format
- Układ informacji komunikatów IBM CICS: https://www.ibm.com/docs/en/cics-ts/6.x?topic=messages-format-cics-message-information
- Format komunikatów IBM MQ for z/OS: https://www.ibm.com/docs/SSFKSJ_9.2.0/com.ibm.mq.ref.doc/q050270_.htm
- Wytyczne Microsoft dotyczące komunikatów błędów: https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error

## Format

```text
DBPnnnnS message text. Next: corrective action.
```

Pola:

- `DBP` oznacza DBWarp Blueprint.
- `nnnn` jest stabilnym czterocyfrowym numerem komunikatu.
- `S` oznacza ważność: `E` błąd, `W` ostrzeżenie, `I` informacja.

Kod jest stabilny i niezależny od języka. Jego podsumowanie, przyczyna i
działanie naprawcze są lokalizowane, gdy `--lang` lub ustawienia regionalne
procesu wybierają obsługiwany język. Dynamiczne szczegóły systemu operacyjnego,
sterownika bazy danych, ścieżki i łańcucha przyczyn pozostają niezmienione, aby
inżynierowie pomocy technicznej mogli wyszukać oryginalną awarię. Tekst
komunikatu nie może zawierać sekretów ani URI połączeń, z których nie usunięto danych wrażliwych.

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
| `DBP1006E` | Zażądano próbkowania plików strukturalnych bez jawnego `--yes`. |
| `DBP1007E` | Zażądano jawnego trybu wierności długości dla silnika, który nie udostępnia jeszcze tego kontraktu. |
| `DBP1008E` | Starszy alias dokładnej długości jest sprzeczny z rygorystyczną wiernością długości. |
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
| `DBP1402E` | Awaria na granicy przechwytywania MySQL lub MariaDB. |
| `DBP1403E` | Awaria na granicy przechwytywania SQL Server. |
| `DBP1404W` | Tryb PostgreSQL TLS `prefer` przeszedł na tekst jawny dla pętli zwrotnej. |
| `DBP1405W` | Opcjonalny pomiar RTT bazy danych był niedostępny. |
| `DBP1406W` | Wyczerpano budżet czasu próbkowania poziomu 2. |
| `DBP1407W` | Próbka kompresji była niedostępna. |
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
| `DBP1501E` | Awaria na granicy przechwytywania pliku strukturalnego. |
| `DBP1502E` | Niepowodzenie danych wyjściowych Blueprint lub pakietu. |
| `DBP1503E` | Niepowodzenie generowania prezentacji PowerPoint. |
| `DBP1504W` | Nie można zapisać dziennika audytu. |
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

Każdy reklamowany język musi zawierać podsumowanie, przyczynę i działanie dla
każdego bieżącego kodu DBP. Plik binarny sprawdza to podczas uruchamiania i
kończy się błędem `DBP1010E`, zamiast po cichu wracać do języka angielskiego.

Przewidywalne awarie na granicach decyzji są sprawdzane przez adwersarialną
macierz testów CLI. Znany warunek musi emitować swój konkretny kod jako pierwszy kod
operatorski i nie może wracać do `DBP0001E`. Moduł renderujący przegląda również
pełny łańcuch błędów, dzięki czemu niekodowany kontekst implementacji nie może
ukryć zakodowanej przyczyny wewnętrznej.

Niekrytyczne ostrzeżenia dotyczące próbkowania bazy danych są wypisywane ze
stabilnym kodem ostrzeżenia i zapisywane w audycie uruchomienia. Pozwala to
odróżnić pełne przechwycenie poziomu 2 od udanego, ale częściowo próbkowanego
przechwycenia, bez przekształcania awarii opcjonalnego pomiaru w całkowitą
awarię zbierania.

## Lista kontrolna dla pomocy technicznej

Gdy klient zgłasza awarię, poproś o:

- kompletne dane wyjściowe terminala, w tym kod `DBP`;
- dziennik audytu, jeśli użyto `--audit-log`;
- zredagowany wiersz polecenia;
- w przypadku błędów pakietu dane wyjściowe `dbwarp-blueprint --bundle-list ...`.

Nie proś o pliki haseł, pliki tokenów, klucze prywatne ani surowe próbki wierszy bazy danych.

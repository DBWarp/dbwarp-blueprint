# Szybki start

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../QUICKSTART.md).

[English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [简体中文](../zh/QUICKSTART.md)

Ten przewodnik szybkiego startu jest przeznaczony dla administratora baz danych (DBA) lub osoby odpowiedzialnej za weryfikację bezpieczeństwa, które muszą utworzyć plik DBWarp Blueprint, który można udostępnić, bez ujawniania danych.

## 1. Wybierz sposób uruchomienia narzędzia

Użyj jednej z następujących ścieżek:

- Pobierz wydany plik binarny i zweryfikuj jego sumę kontrolną.
- Zbuduj ze źródeł za pomocą `./build.sh`.
- Zbuduj z dostarczonego z wydaniem pakietu źródeł z zależnościami do rygorystycznego przeglądu zależności offline.

Zobacz [`BUILD.md`](BUILD.md) i [`binaries/README.md`](BINARIES.md).

W razie potrzeby jawnie wybierz język prezentacji:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --schema app --dry-run
```

Obsługiwane wartości to `en`, `de`, `fr`, `es`, `pl`, `ja` i `zh`. Język
prezentacji zmienia pomoc, monity, diagnostykę, tekst postępu i treść
prezentacji. Nigdy nie zmienia nazw opcji, akceptowanych wartości, schematów
URI, selektorów, kodów DBP, kluczy audytu ani pliku TOML Blueprint. Zobacz
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Utwórz dedykowane konto z minimalnymi uprawnieniami

Zrób to przed każdym połączeniem z bazą, również przed przykładami
`--dry-run`, które później zostaną użyte do przechwycenia danych. Nie zaczynaj
od konta właściciela aplikacji, administratora, superużytkownika, `root`, `sa`
ani `db_owner`.

1. Określ dokładny silnik i jego wersję, bazę danych oraz zatwierdzony schemat
   lub schematy.
2. Wybierz poziom zbierania danych: `basic` tylko dla katalogów tabel, `standard` aby dodać ograniczoną próbkę wierszy lub `enhanced` dla analizy obiektów innych niż tabele.
3. Poproś DBA o skopiowanie odpowiedniego skryptu z
   `sql/grants/<engine>/`, edycję wszystkich oznaczonych wartości bazy,
   schematu, podmiotu, hasła i przełącznika roli oraz uruchomienie go w ramach
   standardowego procesu zarządzania zmianami.
4. Używaj utworzonego konta dedykowanego i w każdym poleceniu działającym na
   bazie przekaż ten sam zatwierdzony zakres, podając po jednej opcji
   `--schema NAME` dla każdego schematu.
5. Po przejrzeniu zebranych danych poproś administratora bazy danych (DBA), aby sprawdził i uruchomił z katalogu `sql/revoke/` skrypt odwołujący uprawnienia, który odpowiada wybranemu silnikowi, w celu usunięcia konta i uprawnień.

Skrypty celowo rozróżniają precyzyjnie ograniczone uprawnienia i wygodniejsze
role wbudowane oraz wyjaśniają, kiedy rola jest szersza. Skrypty wykonywalne
opisuje [`../../sql/grants/README.md`](../../sql/grants/README.md), a
uzasadnienie zależne od wersji dla DBA i zespołu bezpieczeństwa —
[`../../sql/grants/DATABASE_PERMISSIONS.md`](../../sql/grants/DATABASE_PERMISSIONS.md).
Sam kolektor nie tworzy, nie rozszerza ani nie usuwa podmiotów bazy danych.

## 3. Bezpiecznie przygotuj poświadczenia

Nie umieszczaj haseł w URI połączenia. Narzędzie odrzuca hasła osadzone w URI, aby zapobiec wyciekom przez listę procesów i historię powłoki.

Preferowany wzorzec z plikiem hasła (sekret jest wprowadzany bez echa i nie
pojawia się w historii powłoki):

```bash
sudo install -d -m 700 -o "$USER" -g "$(id -gn)" /etc/dbwarp
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Jeżeli nazwa użytkownika jest trudna do zakodowania w URI, również umieść ją w pliku:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\migration_user' > /etc/dbwarp/db.user
```

Następnie użyj `--user-file /etc/dbwarp/db.user`.

## 4. Najpierw wykonaj przebieg próbny

Przebieg próbny sprawdza argumenty i wypisuje planowane działanie bez nawiązywania połączenia:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

Dla trybu prezentacji `--from-toml` przebieg próbny jest lokalną kontrolą wstępną i nie odczytuje bazy danych.

Dla wielu źródeł, zamiast tego, przeprowadź test bez faktycznego działania dla całego manifestu:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 5. Uruchom tryb wyłącznie katalogowy

Tryb wyłącznie katalogowy odczytuje metadane i statystyki, ale nie próbki wierszy:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Użyj tego trybu, gdy zasady zabraniają próbkowania wierszy albo gdy chcesz przeprowadzić pierwszy przegląd bezpieczeństwa.

## 6. Wybierz szczegółowość artefaktów innych niż tabele

Domyślne `--artifact-detail summary` odczytuje katalogi obiektów innych niż tabele, ale nie definicje. Emituje ograniczone liczniki i klasy zewnętrznych wymagań. Użyj `--artifact-detail none`, jeśli zasady zabraniają odczytu tych katalogów. Sonda topologii ograniczona do zliczania nadal działa; zobacz [opis uprawnień](../../sql/grants/README.md#topology-evidence).

Dla anonimowej topologii zależności użyj `graph`. Dla ograniczonych przedziałów cech języka i złożoności użyj `analyzed`. Oba wymagają wyraźnej zgody:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


Dane wyjściowe nigdy nie zawierają nazw obiektów, tekstu definicji, punktów końcowych, sekretów, kluczy, certyfikatów ani plików binarnych. Przed zatwierdzeniem trybu graph lub analyzed przeczytaj [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## 7. Wykonaj pomiar kompresji poziomu 2

Poziom 2 odczytuje ograniczone próbki wierszy do pamięci, oblicza zagregowane
pomiary kompresji, gęstości NULL, kardynalności/częstotliwości, długości i stylu,
a następnie usuwa próbkowane wartości:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

Używaj poziomu 2, jeśli to możliwe. Zapewnia on dokładniejsze oszacowania rozmiaru transferu i kosztów transferu danych.

## 8. Wygeneruj prezentację

Podczas pracy na żywo:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --audit-log blueprint.audit.txt \
  --yes
```

Albo po przeglądzie, bez połączenia z bazą danych:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

## 9. Sprawdź przed udostępnieniem

Sprawdź:

```bash
less blueprint.toml
less blueprint.audit.txt
unzip -l blueprint.pptx  # optional deck package inspection
```

Oczekiwane właściwości:

- brak rzeczywistych nazw tabel;
- brak rzeczywistych nazw kolumn;
- brak wartości wierszy;
- brak komentarzy poza stałym nagłówkiem;
- zaokrąglone liczby i rozmiary w bajtach;
- zanonimizowane identyfikatory, takie jak `table-001`, `col-1` i `schema-A`;
- ograniczone liczniki artefaktów i, po zatwierdzeniu, anonimowe identyfikatory artefaktów;
- jawne dowody niekompletności lub nieczytelności artefaktów zamiast cichego pomijania;
- opcjonalne zagregowane pomiary kompresji, gęstości NULL,
  kardynalności/częstotliwości, długości i stylu, nigdy próbkowane wartości.

## 10. Udostępnij z DBWarp.

Minimum do udostępnienia:

```text
blueprint.toml
```

Dla wielu źródeł, zamiast udostępniać katalog roboczy, należy utworzyć i sprawdzić spakowany zestaw danych.

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Metadane pakietu zachowują identyfikatory źródeł, tagi i identyfikatory grup
zestawów danych wybrane w manifeście wsadowym. Używaj anonimowych wartości i
sprawdź je przed przekazaniem.

Sprawdź [Pakiety zbiorów i blueprintów](BATCH_AND_BUNDLES.md), jeśli masz kilka baz danych lub kilka zestawów danych w formacie Parquet lub Avro, lub chcesz udostępnić tylko wybrane źródła lub tabele.

### Sprawdź i udostępnij

Domyślnie udostępniaj tylko sprawdzony `blueprint.toml` lub spakowany pakiet. Prezentację można dołączyć wyłącznie po sprawdzeniu jej treści i oznaczenia poufności oraz odrębnym zatwierdzeniu zgodnie z polityką organizacji.

Zachowuj audyty, zapisy poleceń oraz niezatwierdzone prezentacje lokalnie i ogranicz dostęp do nich. Mogą one zawierać adresy końcowe, uwierzytelnione podmioty, lokalne ścieżki, dane dotyczące czasu oraz identyfikatory manifestów. Przekazuj je tylko w konkretnych przypadkach potrzeby serwisowej, za pośrednictwem zatwierdzonego, bezpiecznego kanału. Nigdy nie dołączaj plików z hasłami lub tokenami, kluczy anonimizacji, prywatnych kluczy CA, kopii zapasowych baz danych ani logów baz danych do współdzielonego Blueprint.

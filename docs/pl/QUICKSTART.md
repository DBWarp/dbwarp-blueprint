# Szybki start

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../QUICKSTART.md).

[English](../QUICKSTART.md) | [Deutsch](../de/QUICKSTART.md) | [Français](../fr/QUICKSTART.md) | [Español](../es/QUICKSTART.md) | [Polski](QUICKSTART.md) | [日本語](../ja/QUICKSTART.md) | [简体中文](../zh/QUICKSTART.md)

Ten przewodnik szybkiego startu jest przeznaczony dla inżyniera sprzedaży, DBA lub recenzenta bezpieczeństwa, który musi utworzyć możliwy do udostępnienia plik Blueprint DBWarp bez ujawniania danych klienta.

## 1. Wybierz sposób uruchomienia narzędzia

Użyj jednej z następujących ścieżek:

- Pobierz wydany plik binarny i zweryfikuj jego sumę kontrolną.
- Zbuduj ze źródeł za pomocą `./build.sh`.
- Zbuduj z dostarczonego z wydaniem pakietu źródeł z zależnościami do rygorystycznego przeglądu zależności offline.

Zobacz [`BUILD.md`](BUILD.md) i [`binaries/README.md`](BINARIES.md).

W razie potrzeby jawnie wybierz język prezentacji:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --dry-run
```

Obsługiwane wartości to `en`, `de`, `fr`, `es`, `pl`, `ja` i `zh`. Język
prezentacji zmienia pomoc, monity, diagnostykę, tekst postępu i treść
prezentacji. Nigdy nie zmienia nazw opcji, akceptowanych wartości, schematów
URI, selektorów, kodów DBP, kluczy audytu ani pliku TOML Blueprint. Zobacz
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Bezpiecznie przygotuj poświadczenia

Nie umieszczaj haseł w URI połączenia. Narzędzie odrzuca hasła osadzone w URI, aby zapobiec wyciekom przez listę procesów i historię powłoki.

Preferowany wzorzec z plikiem hasła (sekret jest wprowadzany bez echa i nie
pojawia się w historii powłoki):

```bash
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

Jeżeli nazwa użytkownika jest trudna do zakodowania w URI, również umieść ją w pliku:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\\migration_user' > /etc/dbwarp/db.user
```

Następnie użyj `--user-file /etc/dbwarp/db.user`.

## 3. Najpierw wykonaj przebieg próbny

Przebieg próbny sprawdza argumenty i wypisuje planowane działanie bez nawiązywania połączenia:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

Dla trybu prezentacji `--from-toml` przebieg próbny jest lokalną kontrolą wstępną i nie odczytuje bazy danych.

W przypadku wielu źródeł klienta wykonaj zamiast tego przebieg próbny manifestu wsadowego:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 4. Uruchom tryb wyłącznie katalogowy

Tryb wyłącznie katalogowy odczytuje metadane i statystyki, ale nie próbki wierszy:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Użyj tego trybu, gdy zasady zabraniają próbkowania wierszy albo gdy chcesz przeprowadzić pierwszy przegląd bezpieczeństwa.

## 5. Wybierz szczegółowość artefaktów innych niż tabele

Domyślne `--artifact-detail summary` odczytuje katalogi obiektów innych niż tabele, ale nie definicje. Emituje ograniczone liczniki i klasy zewnętrznych wymagań. Użyj `--artifact-detail none`, jeśli zasady zabraniają odczytu tych katalogów.

Dla anonimowej topologii zależności użyj `graph`. Dla ograniczonych przedziałów cech języka i złożoności użyj `analyzed`. Oba wymagają wyraźnej zgody:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```


Dane wyjściowe nigdy nie zawierają nazw obiektów, tekstu definicji, punktów końcowych, sekretów, kluczy, certyfikatów ani plików binarnych. Przed zatwierdzeniem trybu graph lub analyzed przeczytaj [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## 6. Wykonaj pomiar kompresji poziomu 2

Poziom 2 odczytuje ograniczone próbki wierszy do pamięci, kompresuje je lokalnie, zapisuje wyłącznie współczynniki podsumowujące i usuwa bajty próbek:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

W miarę możliwości używaj poziomu 2. Zapewnia DBWarp lepsze oszacowania liczby bajtów przesyłanych siecią, kosztu ruchu wychodzącego oraz generowania syntetycznych danych tekstowych i binarnych.

## 7. Wygeneruj prezentację

Podczas pracy na żywo:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

## 8. Sprawdź przed udostępnieniem

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
- opcjonalnie wyłącznie współczynniki kompresji, bez bajtów próbek.

## 9. Przekaż dane do DBWarp

Minimalny pakiet do przekazania:

```text
blueprint.toml
```

W przypadku przeglądu klienta obejmującego wiele źródeł utwórz i sprawdź
spakowany pakiet, zamiast przekazywać katalog roboczy:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Metadane pakietu zachowują identyfikatory źródeł, tagi i identyfikatory grup
zestawów danych wybrane w manifeście wsadowym. Używaj anonimowych wartości i
sprawdź je przed przekazaniem.

Skorzystaj z `docs/BATCH_AND_BUNDLES.md`, gdy klient ma wiele baz danych, wiele zestawów danych Parquet lub Avro albo chce zatwierdzić tylko wybrane źródła lub tabele do wygenerowania benchmarku.

Domyślnie zachowaj następujące elementy lokalnie jako dowody z kontrolą dostępu:

```text
blueprint.audit.txt
blueprint.pptx
command-used.txt
```

Audyty i zapisane polecenia mogą zawierać punkty końcowe baz danych,
uwierzytelnione podmioty, lokalne ścieżki, dane czasowe i identyfikatory źródeł
manifestu. Wysyłaj je tylko w odpowiedzi na konkretną potrzebę pomocy technicznej
zatwierdzonym bezpiecznym kanałem. Nie wysyłaj plików haseł, prywatnych kluczy
CA, zrzutów danych klienta ani dzienników bazy danych.

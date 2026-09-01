<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="../../.github/assets/dbwarp-logo-dark.png">
    <img src="../../.github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

# dbwarp-blueprint

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../README.md). Zobacz [zasady tłumaczenia dokumentacji](../TRANSLATIONS.md).

[English](../../README.md) | [Deutsch](../de/README.md) | [Français](../fr/README.md) | [Español](../es/README.md) | [Polski](README.md) | [日本語](../ja/README.md) | [简体中文](../zh/README.md)

Angielska dokumentacja jest rozstrzygająca. Zestawy dokumentów tłumaczone
maszynowo mogą być publikowane oddzielnie dopiero po kilku niezależnych
przeglądach, ale nadal mogą zawierać błędy.

## Czym jest to narzędzie

DBWarp Blueprint to kolektor Blueprint bazy danych zaprojektowany z myślą o zaufaniu. Uruchamiasz go we własnym środowisku względem PostgreSQL, MySQL lub SQL Server. Odczytuje metadane katalogowe, a tylko na żądanie pomiaru kompresji również ograniczoną próbkę wierszy. Następnie zapisuje zanonimizowany strukturalny Blueprint bazy danych: rozmiary tabel, liczby wierszy, rodziny typów oraz strukturę indeksów i kluczy obcych.

Identyfikatory są zastępowane anonimowymi etykietami chronionymi kluczem, a wartości wierszy nie są zapisywane w pliku Blueprint. Domyślnie nowy klucz lokalny dla procesu uniemożliwia sprawdzanie słownikowe offline; `--anonymization-key-file` pozwala klientowi zachować etykiety między zatwierdzonymi uruchomieniami porównawczymi. Przed udostępnieniem wyników przeczytaj [`SECURITY.md`](SECURITY.md): dokument ten dokładnie określa, co ujawnia każdy tryb i które opcje zwiększają zakres ujawnianych informacji.

Wynik jest plikiem tekstowym. Przed podjęciem decyzji o jego udostępnieniu możesz przeczytać każdy wiersz.

DBWarp Blueprint jest bezpłatnym oprogramowaniem typu open source i działa w całości w Twoim środowisku. Powstał po to, aby umożliwić przekazanie nam faktów o bazie danych bez przekazywania samej bazy danych.

## Dlaczego warto go uruchomić

Udostępnij nam wynik Blueprint, a będziemy mogli określić, o ile szybciej DBWarp przeniósłby Twoje dane i jaki miałoby to wpływ na harmonogram migracji, przygotowywania danych testowych CI/CD oraz analiz.

Najważniejsza jest odległość. Im dalej muszą zostać przesłane dane, tym większą poprawę może wykazać DBWarp.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zurych, Szwajcaria

---

`dbwarp-blueprint` jest działającym po stronie klienta kolektorem Blueprint dla DBWarp. Uruchamia się go we własnym środowisku klienta, aby utworzyć oczyszczony, możliwy do przeglądu plik `blueprint.toml`, którego DBWarp może użyć do wymiarowania migracji, generowania syntetycznych zestawów danych i planowania wstępnego bez otrzymywania dostępu do bazy danych, zrzutów, nazw schematów ani danych wierszy.

Narzędzie łączy się z PostgreSQL, MySQL lub SQL Server, odczytuje metadane katalogu, opcjonalnie mierzy lokalną kompresję na podstawie ograniczonej próbki wierszy i zapisuje TOML w zwykłym tekście. Może również wyprowadzić Blueprint z lokalnych plików Parquet lub Avro w trybie offline, gdy dane wejściowe są już plikiem strukturalnym, a nie bazą danych na żywo. Możesz otworzyć dane wyjściowe, przejrzeć każdy wiersz i zdecydować, czy je udostępnić.

Opcjonalnie `--deck blueprint.pptx` zapisuje również podsumowanie PowerPoint tego samego zanonimizowanego Blueprint. Prezentację można wygenerować podczas pracy z bazą danych na żywo albo później ze sprawdzonego pliku TOML za pomocą `--from-toml blueprint.toml --deck blueprint.pptx`. Generator prezentacji jest wbudowany w plik binarny Rust i nie nawiązuje połączeń sieciowych.

## Do czego służy

DBWarp potrzebuje wystarczającej ilości informacji strukturalnych, aby oszacować i zaplanować transfer:

- liczby tabel;
- przybliżonej liczby wierszy;
- rozmiarów tabel i indeksów;
- rodzin typów kolumn, dokładnych pojemności strukturalnych/prefiksów indeksów
  oraz domyślnie zaokrąglonych dla prywatności zaobserwowanych szerokości;
- struktury indeksów i kluczy obcych;
- bezpiecznych dla prywatności liczników artefaktów innych niż tabele i zewnętrznych wymagań wdrożeniowych;
- opcjonalnych podsumowań kompresji tabel i kolumn z małej lokalnej próbki;
- opcjonalnych dowodów RTT bazy danych po stronie klienta.

Te fakty wystarczają do oszacowania wielkości transferu, wybrania początkowego planu transferu masowego DBWarp i wygenerowania reprezentatywnego syntetycznego zestawu testowego. Nie wystarczają do odtworzenia schematu ani danych klienta.

## Czego nie robi

Narzędzie `dbwarp-blueprint`:

- nie wysyła telemetrii;
- nie wywołuje serwerów DBWarp;
- nie przesyła pliku Blueprint;
- nie odczytuje `~/.pgpass`, `~/.my.cnf`, poświadczeń chmurowych ani kluczy SSH;
- nie odczytuje domyślnych zmiennych środowiskowych haseł, takich jak `PGPASSWORD` lub `MYSQL_PWD`;
- nie zapisuje niczego poza wynikami wybranymi dla aktywnego trybu; tryb wsadowy
  zapisuje katalog pakietu z podrzędnymi plikami Blueprint, podrzędnymi audytami
  i opcjonalnymi dowodami błędów;
- nie umieszcza w danych wyjściowych rzeczywistych nazw tabel, kolumn, indeksów ani schematów, nazw obiektów innych niż tabele, definicji SQL, zewnętrznych punktów końcowych, poświadczeń, kluczy, certyfikatów, plików binarnych ani wartości wierszy.

Uruchomienie Blueprint na żywo otwiera sesję bazy danych z podanym punktem
końcowym. DNS może korzystać ze skonfigurowanego resolvera, a zintegrowane
uwierzytelnianie Kerberos/SSPI może kontaktować się z infrastrukturą tożsamości.
Tryb wsadowy powtarza tę granicę dla każdego źródła bazodanowego. Lokalne
operacje TOML, Parquet, Avro i operacje na pakietach nie inicjują połączeń
sieciowych aplikacji.

## Pobieranie lub budowanie

| Ścieżka | Najlepsze zastosowanie | Odsyłacz |
|---|---|---|
| Pobranie pliku binarnego | szybka próba, rozmowa z inżynierem sprzedaży, izolowany host testowy | [`binaries/README.md`](BINARIES.md) |
| Budowanie z małego klonu źródeł | przegląd bezpieczeństwa, zasady produkcyjne, kontrola odtwarzalności | [`BUILD.md`](BUILD.md) |
| Budowanie z pakietu źródeł z zależnościami | rygorystyczny audyt zależności offline | GitHub Releases |

Ścieżką stawiającą zaufanie na pierwszym miejscu jest budowanie ze źródeł. Zwykłe repozytorium pozostaje małe i używa `Cargo.lock` do przypięcia wersji zależności. Na potrzeby bardziej rygorystycznych audytów offline każde wydanie publikuje również pakiet źródeł ze wszystkimi plikami źródłowymi zależności. Dla wygody dostarczane są pliki binarne wydania z sumami kontrolnymi SHA256.

## Szybki start

W razie potrzeby wybierz język prezentacji. Domyślny jest angielski; kompletne
katalogi są wbudowane dla języka niemieckiego, francuskiego, hiszpańskiego,
polskiego, japońskiego i chińskiego uproszczonego:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --dry-run
```

Tłumaczone są wyłącznie pomoc, monity, diagnostyka, postęp i etykiety
prezentacji PowerPoint przeznaczone dla człowieka. Nazwy poleceń i opcji,
akceptowane wartości, schematy URI, nazwy zmiennych środowiskowych, selektory,
kody DBP, klucze audytu i generowany TOML pozostają kanonicznymi tokenami
angielskimi. Dzięki temu automatyzacja i procedury pomocy są identyczne w każdym
języku. Zobacz [`docs/INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

Najpierw wykonaj przebieg próbny. Wypisuje plan bez nawiązywania połączenia:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --dry-run
```

Zalecane uruchomienie w stylu produkcyjnym z TLS, dziennikiem audytu i pomiarem kompresji:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Z `--measure-compression --yes` dane wyjściowe zawierają współczynniki zstd na
poziomie tabeli i projekcje kompresji dla poszczególnych kolumn. Bloki kolumn są
obliczane z tej samej ograniczonej próbki co współczynnik tabeli; są przeznaczone
do estymacji syntetycznych zestawów testowych DBWarp i nie zapisują wartości próbek na dysku.
Schemat v3 i nowsze emitują również bezpieczne agregaty kardynalności i rozkładu dla
kolumn oraz wywnioskowane podsumowania prefiksów indeksów i relacji. Tymczasowe
odciski są ograniczone w pamięci i usuwane; wartości i odciski nigdy nie trafiają
do pliku TOML Blueprint.

Od schematu v4 Blueprinty inwentaryzują również obiekty inne niż tabele. Domyślne
`--artifact-detail summary` zapisuje ograniczone liczniki według klas obiektów i
zewnętrznych wymagań bez odczytywania definicji. `graph` dodaje anonimową
topologię zależności, a `analyzed` ograniczone przedziały cech języka i
złożoności. Oba wymagają `--yes`, ponieważ nawet anonimowy graf może
identyfikować aplikację:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```


Obecność artefaktu jest dowodem planistycznym, a nie deklaracją, że DBWarp może
go automatycznie odtworzyć lub przetłumaczyć. Zobacz
[`docs/ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

### Wierność długości MySQL

Domyślna polityka `balanced` zachowuje dokładnie zadeklarowane pojemności
znakowe/bajtowe i długości prefiksów indeksów. Próbkowane średnie/p95 długości
wartości korzystają z przedziałów błędu względnego (maksymalny błąd około 3,2%,
przy czym wartości do 32 bajtów zachowuje się dokładnie). Dzięki temu klucz
`VARCHAR(3000)`, którego wartości zwykle mają 9 znaków, pozostaje blisko 9
znaków w generowanych danych, a jednocześnie zachowane są prawidłowe limity
DDL/indeksów źródła:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

Używaj dokładnych statystyk próbek tylko wtedy, gdy zasady pozwalają na dodatkową precyzję:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Użyj `--length-fidelity strict`, aby zachować starsze, zgrubne przedziały
bezpieczne do udostępniania dla długości deklarowanych, obserwowanych i
prefiksów. Tryb `strict` celowo poświęca wierność zestawu testowego i indeksu i nie
jest gotowy do benchmarku klienta. Starsza pisownia
`--preserve-exact-lengths --yes` pozostaje aliasem zgodności dla
`--length-fidelity exact --yes`.

Nowe Blueprinty zapisują osobne pola `declared_length_fidelity`,
`index_length_fidelity` i `observed_length_fidelity`. Starsze pole
`length_metadata` pozostaje dla zachowawczej zgodności ze starszymi
konsumentami. Pojemności znakowe PostgreSQL są dokładnymi wartościami
katalogowymi; zależne od kodowania limity bajtowe i długości prefiksów indeksów
pozostają niedostępne.

Dla generowanego benchmarku reprezentatywnego dla klienta
`--measure-compression` nie jest opcjonalne: dostarcza zaobserwowane średnie/p95
długości wartości, dzięki czemu zadeklarowany wielokilobajtowy klucz, którego
rzeczywiste wartości mają tylko kilka znaków, nie jest generowany z długością
równą pojemności. Domyślny budżet czasu rzeczywistego próbkowania wynosi 300
sekund. Zwiększ `--max-wall-secs` dla bardzo dużych schematów. Narzędzia
planistyczne działające dalej powinny odrzucić Blueprint, jeśli którakolwiek
niepusta indeksowana kolumna o zmiennej szerokości pozostanie bez próbki.
Generowanie na potrzeby testu dymnego lub zgodności wymaga wtedy jawnego
nadpisania w narzędziu dalszego etapu i musi zostać oznaczone jako
niereprezentatywne.

Następnie przejrzyj pliki:

```bash
less blueprint.toml
less audit.txt
```

Jeżeli jest to zgodne z zasadami, udostępnij `blueprint.toml` firmie DBWarp.
Prezentację również można udostępnić po przeglądzie. Dziennik audytu przechowuj
jako dowód operacyjny z kontrolą dostępu, chyba że konkretny przypadek pomocy
technicznej wymaga przekazania go zatwierdzonym bezpiecznym kanałem; zawiera
punkt końcowy, tożsamość, ścieżki i szczegóły czasowe.

## Tryb pliku strukturalnego

Jeżeli źródłem jest już lokalny plik strukturalny, wygeneruj TOML Blueprint bez poświadczeń bazy danych:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Tryb Parquet odczytuje stopkę i metadane grup wierszy. Kontenery obiektów Avro nie mają równoważnej liczby wierszy w stopce, dlatego tryb Avro przechodzi przez kontener, aby policzyć rekordy, i używa schematu zapisu (writer schema) do określenia struktury kolumn. Żaden z trybów nie łączy się z bazą danych ani nie odczytuje flag poświadczeń.

Jeżeli zasady zezwalają na zdekodowane próbkowanie, tryb plikowy może również
oszacować kompresję podobną do transportu DBWarp na podstawie ograniczonych
próbek lokalnych:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

Te same flagi działają z `--from-avro`. Próbkowane wartości są kodowane w
pamięci jako `dbwarp-blueprint-rowframe-v1`; do TOML Blueprint trafiają tylko
zagregowane współczynniki kompresji zstd.

## Tryb wsadowy i pakietowy

Dla wielu baz danych, wielu tabel/zestawów danych albo przeglądu środowiska klienta użyj manifestu wsadowego i zapisz katalog pakietu:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

Katalog roboczy zawiera `bundle.toml`, podrzędne pliki Blueprint dla
poszczególnych źródeł oraz objęte kontrolą dostępu dzienniki audytu. Domyślnie
nie przekazuj całego katalogu roboczego. Możesz wyświetlić lub wyodrębnić jego
zawartość albo utworzyć oddzielnie sprawdzony, spakowany pakiet Blueprint:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

Składnia manifestu, tryby zestawów plików strukturalnych i zasady selektorów są
opisane w [`docs/BATCH_AND_BUNDLES.md`](BATCH_AND_BUNDLES.md).

## Typowe polecenia dla baz danych

PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

Przykłady Kerberos, SSPI i Entra ID znajdują się w [`AUTH.md`](AUTH.md). Informacje o wewnętrznych CA, mTLS i weryfikacji nazwy hosta znajdują się w [`TLS.md`](TLS.md).

## Tryb wyłącznie katalogowy

Jeżeli zasady zabraniają próbkowania wierszy, pomiń `--measure-compression`:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

Tryb wyłącznie katalogowy odczytuje tylko metadane. DBWarp nadal może estymować na podstawie rozmiaru tabel, liczby wierszy, rodzin typów i struktury indeksów/FK, ale kompresja i realizm syntetycznego zestawu danych są słabsze, ponieważ entropia tekstu/danych binarnych musi zostać wywnioskowana.

## Podgląd danych wyjściowych

```toml
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 6
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 28
row_count = 12500000
table_bytes = 4200000000
index_bytes = 1100000000

[tables.table-001]
rows = 12500000
table_bytes = 4200000000
index_bytes = 1100000000
schema = "schema-A"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

Pełny kontrakt pliku opisano w [`FORMAT.md`](FORMAT.md). Dziennik audytu opisano w [`AUDIT.md`](AUDIT.md).

## Wizualna prezentacja podsumowująca

Wygeneruj prezentację podczas uruchomienia na żywo:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

Albo zbuduj ją później ze sprawdzonego pliku Blueprint, bez połączenia z bazą danych:

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

Prezentacja dostosowuje się do wielkości schematu: szczegóły poszczególnych tabel dla małych schematów, slajdy charakteryzujące dla dużych schematów, podsumowanie kompresji, gdy dostępne są dane poziomu 2, oraz slajd modelu zaufania. Zobacz [`DECK.md`](DECK.md).

## Dokumentacja

Zacznij tutaj:

- [`docs/QUICKSTART.md`](QUICKSTART.md): pierwsze bezpieczne uruchomienie i pierwszy pakiet do przekazania.
- [`docs/COOKBOOK.md`](COOKBOOK.md): praktyczne przepisy dla PostgreSQL, MySQL, SQL Server, TLS, prezentacji i przepływów bez próbkowania.
- [`docs/DBA_REVIEW_GUIDE.md`](DBA_REVIEW_GUIDE.md): informacje potrzebne DBA lub recenzentowi bezpieczeństwa przed uruchomieniem narzędzia.
- [`sql/grants/README.md`](../../sql/grants/README.md): uwzględniające wersję skrypty minimalnych uprawnień i usuwanie konta po przechwyceniu.
- [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md): typowe awarie i sposoby ich rozwiązania.
- [`docs/MESSAGES.md`](MESSAGES.md): stabilne kody komunikatów operatorskich `DBPnnnnS`.
- [`docs/COMPRESSION_MEASUREMENT.md`](COMPRESSION_MEASUREMENT.md): sposób działania próbkowania kompresji poziomu 2.
- [`docs/INDEX.md`](INDEX.md): kompletna mapa dokumentacji.

Punkty wyjścia do przeglądu bezpieczeństwa:

- [`SECURITY.md`](SECURITY.md): model bezpieczeństwa i obsługa poświadczeń.
- [`AUDIT.md`](AUDIT.md): co jest odczytywane, zapisywane, odpytywane i rejestrowane.
- [`FORMAT.md`](FORMAT.md): pola danych wyjściowych i zasady zaokrąglania.
- [`TLS.md`](TLS.md): zachowanie TLS i mTLS.
- [`AUTH.md`](AUTH.md): obsługiwane tryby uwierzytelniania.
- [`BUILD.md`](BUILD.md): budowanie ze źródeł i weryfikacja wydania.
- [`DECK.md`](DECK.md): opcjonalna prezentacja podsumowująca PowerPoint.

## Licencja

Apache-2.0 OR MIT.

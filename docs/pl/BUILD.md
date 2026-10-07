# Budowanie dbwarp-blueprint ze źródeł

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../BUILD.md).

[English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | [Français](../fr/BUILD.md) | [Español](../es/BUILD.md) | [Polski](BUILD.md) | [日本語](../ja/BUILD.md) | [简体中文](../zh/BUILD.md)

Użyj tego przewodnika, jeśli wolisz samodzielnie zbudować narzędzie przed
uruchomieniem go względem bazy danych.

## Szybkie budowanie

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
./build.sh
```

Plik binarny jest zapisywany w:

```text
target/release/dbwarp-blueprint
```

Pozostałe przykłady używają `./dbwarp-blueprint`. Po kompilacji ze źródeł uruchom
bezpośrednio `target/release/dbwarp-blueprint` albo skopiuj ten plik do
`./dbwarp-blueprint`, zanim wykonasz przykłady.

Jeśli przypięty toolchain Rust nie jest zainstalowany, a zatwierdzono dostęp do
sieci, wyraźnie wyraź zgodę:

```bash
ALLOW_NETWORK=1 ./build.sh
```

## Co robi skrypt budowania

`build.sh` jest celowo konserwatywny:

- odczytuje przypiętą wersję Rust z `rust-toolchain.toml`;
- używa istniejącego `rustc`, jeśli odpowiada on przypiętej wersji;
- odmawia pobrania Rust, chyba że ustawiono `ALLOW_NETWORK=1`;
- przypina wersję programu rozruchowego rustup i przed użyciem weryfikuje jego oficjalny SHA-256;
- przechowuje stan łańcucha narzędzi pod `./build/`;
- używa Cargo.lock w celu zapewnienia powtarzalnych wersji zależności;
- domyślnie buduje za pomocą `cargo build --release --locked`;
- automatycznie przełącza się na `--frozen --offline --locked`, gdy jest
  uruchamiany z pakietu źródłowego zawierającego zależności;
- odrzuca `DBWARP_BLUEPRINT_OFFLINE=1`, jeśli nie istnieje `vendor-crates/`;
- wyświetla SHA256 wynikowego pliku binarnego;
- zapisuje w audycie dokładną rewizję źródła i stan drzewa roboczego.

Nie używa `sudo` i nie modyfikuje systemowej instalacji Rust.

## Pliki binarne do pobrania

Dla powtarzalnego uruchomienia przypnij dokładny znacznik wydania i zweryfikuj jego SHA-256; nie używaj zmiennego adresu pobierania.

Gotowe pliki binarne są dostępne na stronie Releases:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Są udostępnione dla wygody. Jeżeli Twoje zasady wymagają przeglądu źródeł,
zbuduj program lokalnie z tego samego znacznika.

Archiwa binarne platform są pakietami operatorskimi, a nie drzewami źródeł,
więc nie można w nich ponownie zbudować programu. Zawarte w nich kopie tego
przewodnika i `verify.sh` opisują ścieżkę weryfikacji z użyciem pasującego
źródła. Użyj drzewa dokładnego tagu wydania lub archiwum wydania ze źródłami i
zależnościami, gdy potrzebujesz `build.sh`, źródeł Cargo albo lokalnej
kompilacji porównawczej.

Pliki wydania:

| Platforma | Plik |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Weryfikowanie pobranego archiwum

Linux:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS:

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

Porównaj wyświetloną wartość z odpowiednim wierszem w `SHA256SUMS.txt`.

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Każde wydanie publikuje również plik
`dbwarp-blueprint-<platform>.binary.sha256` dla rozpakowanego pliku
wykonywalnego. Polecenie weryfikacji znajduje się w dokumencie
[Pobieranie plików binarnych](BINARIES.md).

## Kompilacje właściwe dla uwierzytelniania

Domyślna kompilacja obsługuje przepływy hasła, pliku tokenu, zmiennej tokenu i
TLS; mTLS z certyfikatem klienta jest dostępne dla PostgreSQL i MySQL.

Uwierzytelnianie zintegrowane SQL Server jest obsługiwane zależnie od platformy:

| Platforma | Polecenie budowania | Przeznaczenie |
|---|---|---|
| Linux | Plik binarny Linux z wydania GitHub albo `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Uwierzytelnianie hasłem, tokenem i TLS, a także Kerberos / GSSAPI po wybraniu |
| Windows | Plik binarny Windows z wydania GitHub albo `cargo build --release --locked --features winauth` | Windows Integrated Auth / SSPI |

Pliki binarne wydania dla systemu Linux nie wymagają bibliotek Kerberos do
uruchomienia. Ładują środowisko wykonawcze GSSAPI platformy tylko po wybraniu
`--auth-mode integrated`. Jeżeli `kinit` działa, wymagane składniki wykonawcze
są zwykle już dostępne. Kompilacje ze źródeł włączają Kerberos/GSSAPI za pomocą
`integrated-auth-gssapi`, jak pokazano powyżej.

## Budowanie bez skryptu

Jeśli zasady preferują bezpośrednie polecenia Cargo:

```bash
cargo build --release --locked
```

Kompilacja Windows SSPI:

```powershell
cargo build --release --locked --features winauth
```

Kompilacja Linux Kerberos:

```bash
cargo build --release --locked --features integrated-auth-gssapi
```

## Odtwarzanie pliku binarnego wydania

`./build.sh` dowodzi, że sprawdzane źródło kompiluje się poprawnie; dodatkowo, identyczność bajtów wymaga kompletnych, natywnych danych wejściowych do kompilacji wersji. Sprawdź dokładną wersję źródła, zarejestrowaną w `PROVENANCE.json`, publikowaną wraz z każdą wersją, użyj jej listy celów i funkcji, używanego zestawu narzędzi Rust, zarejestrowanych natywnych danych compiler/linker, znacznika czasu commitu `SOURCE_DATE_EPOCH` oraz mapowań ścieżek i flag linkera używanych w procesie tworzenia wersji. Wersje dla Windows również wykorzystują `clang-cl` i `/Brepro`.

Po odtworzeniu tych danych porównaj rozpakowany plik wydania z wynikiem lokalnym:

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

Jeśli sumy kontrolne są różne, nie traktuj plików binarnych jako równoważnych. Każda wersja jest kompilowana dwukrotnie, a niezgodność bajtów powoduje odrzucenie wersji. `PROVENANCE.json` rejestruje wersję źródła, cel, funkcje, środowisko kompilacji, datę kompilacji, kompilator natywny, rozmiar pliku binarnego oraz sumę kontrolną, które są potrzebne do oceny lokalnej reprodukcji.

## Dostarczone zależności

Repozytorium zawiera poprawione zależności w katalogu `vendor/`. Zachowują one
restrykcyjne zasady zaufania `--tls-ca` dla MySQL i SQL Server. Zintegrowane
uwierzytelnianie Linux ładuje GSSAPI tylko wtedy, gdy jest wymagane.
Zintegrowane uwierzytelnianie Windows korzysta z utrzymywanej zależności do
generowania liczb losowych. Wersje wszystkich innych zależności są przypięte
przez `Cargo.lock`.

Każde wydanie GitHub publikuje oddzielny pakiet
`dbwarp-blueprint-source-vendored.tar.gz` dla zespołów bezpieczeństwa, które chcą
sprawdzić i zbudować offline każdy plik źródłowy zależności.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Pakiet ten zawiera poprawione zależności w katalogu `vendor/`, wygenerowane drzewo
`vendor-crates/` dla wszystkich pozostałych zależności oraz wygenerowany plik
`.cargo/config.toml`, który przekierowuje crates.io do lokalnego drzewa
zależności. W tym trybie `build.sh` używa
`cargo build --release --frozen --offline --locked`.

# Budowanie dbwarp-blueprint ze źródeł

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../BUILD.md).

[English](../../BUILD.md) | [Deutsch](../de/BUILD.md) | [Français](../fr/BUILD.md) | [Español](../es/BUILD.md) | [Polski](BUILD.md) | [日本語](../ja/BUILD.md) | [简体中文](../zh/BUILD.md)

Ten przewodnik jest przeznaczony dla klientów, którzy wolą samodzielnie
zbudować narzędzie przed uruchomieniem go względem bazy danych.

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

Pliki wydania:

| Platforma | Plik |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Weryfikowanie pobranego archiwum

Linux/macOS:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

## Kompilacje właściwe dla uwierzytelniania

Domyślna kompilacja obsługuje przepływy hasła, pliku tokenu, zmiennej tokenu i
TLS; mTLS z certyfikatem klienta jest dostępne dla PostgreSQL i MySQL.

Uwierzytelnianie zintegrowane SQL Server ma kompilacje właściwe dla platformy:

| Platforma | Polecenie budowania | Przeznaczenie |
|---|---|---|
| Linux | `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Kerberos / GSSAPI |
| Windows | Plik binarny Windows z wydania GitHub albo `cargo build --release --features winauth` | Windows Integrated Auth / SSPI |

Kerberos w systemie Linux wymaga zwykłych bibliotek wykonawczych MIT Kerberos.
Jeżeli `kinit` działa na hoście, wymagane elementy wykonawcze są zwykle już
dostępne.

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

## Dostarczone zależności

Zwykłe repozytorium zawiera jedną niewielką załataną zależność w
`vendor/mysql_async`, dzięki czemu `--tls-ca` dla MySQL ma tak samo restrykcyjną
semantykę zaufania jak pozostała część narzędzia. Wersje wszystkich innych
zależności są przypięte przez `Cargo.lock`.

Każde wydanie GitHub publikuje oddzielny pakiet
`dbwarp-blueprint-source-vendored.tar.gz` dla zespołów bezpieczeństwa, które chcą
sprawdzić i zbudować offline każdy plik źródłowy zależności.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Pakiet ten zawiera załataną zależność `vendor/mysql_async`, wygenerowane drzewo
`vendor-crates/` dla wszystkich pozostałych zależności oraz wygenerowany plik
`.cargo/config.toml`, który przekierowuje crates.io do lokalnego drzewa
zależności. W tym trybie `build.sh` używa
`cargo build --release --frozen --offline --locked`.

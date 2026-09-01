# Pobieranie dbwarp-blueprint

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../binaries/README.md).

[English](../../binaries/README.md) | [Deutsch](../de/BINARIES.md) | [Français](../fr/BINARIES.md) | [Español](../es/BINARIES.md) | [Polski](BINARIES.md) | [日本語](../ja/BINARIES.md) | [简体中文](../zh/BINARIES.md)

Gotowe pliki binarne `dbwarp-blueprint` są publikowane na stronie GitHub Releases:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

Możesz pobrać plik binarny, zweryfikować jego sumę kontrolną, uruchomić go
lokalnie i sprawdzić wygenerowany plik `blueprint.toml`, zanim udostępnisz cokolwiek
DBWarp.

Wybierz dokładny tag wydania, na przykład `https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0`, a następnie pobierz archiwum i `SHA256SUMS.txt` z tego samego tagu. Do przebiegu odtwarzalnego lub audytowanego nie używaj zmiennego adresu `releases/latest`.

## Pliki

| Platforma | Plik |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |
| Pakiet źródłowy z zależnościami do audytu offline | `dbwarp-blueprint-source-vendored.tar.gz` |
| Sumy kontrolne | `SHA256SUMS.txt` |

Każde wydanie zawiera również plik `SHA256SUMS.txt`.

## Weryfikowanie pobranego pliku

Linux/macOS:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Porównaj wyświetlony skrót z odpowiadającym mu wierszem w `SHA256SUMS.txt`.

## Pobrany plik binarny czy kompilacja lokalna?

Plik binarny do pobrania jest udostępniony dla wygody. Najsilniejszą ścieżką
zaufania pozostaje zbudowanie programu ze źródeł:

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
git checkout <release-tag>
./build.sh
```

Zwykły klon źródeł jest celowo niewielki i używa `Cargo.lock` do przypięcia
wersji zależności.

Jeżeli zasady wymagają przejrzenia każdego pliku źródłowego zależności przed
budowaniem, pobierz `dbwarp-blueprint-source-vendored.tar.gz` z tego samego wydania
i zbuduj program wewnątrz rozpakowanego drzewa:

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

Zobacz [`BUILD.md`](BUILD.md).

## Co robi narzędzie

`dbwarp-blueprint` odczytuje metadane bazy danych i opcjonalnie mierzy kompresję na
małej próbce lokalnej. Zapisuje zanonimizowany plik tekstowy na potrzeby
estymacji migracji DBWarp. Nie przesyła pliku i nie wysyła telemetrii.

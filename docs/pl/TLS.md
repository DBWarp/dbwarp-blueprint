# TLS i certyfikaty

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../../TLS.md).

[English](../../TLS.md) | [Deutsch](../de/TLS.md) | [Français](../fr/TLS.md) | [Español](../es/TLS.md) | [Polski](TLS.md) | [日本語](../ja/TLS.md) | [简体中文](../zh/TLS.md)

Używaj TLS zawsze, gdy połączenie z bazą danych przekracza granicę sieciową.
`verify-full` jest trybem domyślnym: łańcuch certyfikatów i nazwa hosta są weryfikowane, chyba że operator jawnie wybierze inny tryb.

## Typowe opcje

```bash
--tls-mode disable|prefer|require|verify-ca|verify-full
--tls-ca /path/to/ca-bundle.pem
--tls-cert /path/to/client-cert.pem
--tls-key /path/to/client-key.pem
```

Zalecane ustawienie produkcyjne:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

## Wewnętrzny CA

Jeżeli certyfikat bazy danych został podpisany przez wewnętrzny CA:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml
```

## Niezgodność nazwy hosta

Podczas pracy z `--tls-mode verify-full` użyj w `--connect` nazwy hosta zgodnej
z certyfikatem. Ta wersja nie obsługuje nadpisywania nazwy serwera TLS;
przekazanie `--tls-server-name` kończy się wyraźnym błędem zamiast cichego
osłabienia weryfikacji. W przypadku PostgreSQL i MySQL, jeżeli zasady
bezpieczeństwa dopuszczają weryfikację CA bez sprawdzania nazwy hosta, użyj
`--tls-mode verify-ca`.

Domyślne źródła zaufania zależą od silnika:

- PostgreSQL i MySQL używają korzeni Mozilla wkompilowanych w plik binarny, gdy
  pominięto `--tls-ca`. Dostarczony pakiet PEM zastępuje te korzenie.
- SQL Server używa magazynu zaufania systemu operacyjnego, gdy pominięto
  `--tls-ca`. Dostarczony plik `.pem` lub `.crt` musi zawierać dokładnie jeden
  certyfikat CA i zastępuje korzenie systemu operacyjnego.

Sterownik SQL Server sprawdza nazwę hosta połączenia zarówno w trybie
`verify-ca`, jak i `verify-full`; w tym silniku `verify-ca` celowo nie jest
słabsze od `verify-full`.

## Tryby tekstu jawnego i zgodności

`prefer` jest dozwolone tylko dla adresów pętli zwrotnej. PostgreSQL może tam przejść na lokalny tekst jawny i emituje `DBP1404W`; pozostałe silniki nadal próbują TLS. Dla zdalnych celów `disable` i `require` wymagają `--i-know-what-im-doing`, ponieważ zezwalają na tekst jawny albo szyfrują bez uwierzytelnienia serwera. To potwierdzenie nie czyni ich właściwymi dla produkcji.

## mTLS

PostgreSQL i MySQL obsługują uwierzytelnianie certyfikatem klienta. Jeżeli jedna
z tych baz danych wymaga certyfikatu klienta:

```bash
--tls-cert /etc/dbwarp/client.crt \
--tls-key /etc/dbwarp/client.key
```

W systemach uniksopodobnych pliki kluczy prywatnych nie mogą być dostępne do odczytu dla grupy ani innych użytkowników.
Uwierzytelnianie certyfikatem klienta w SQL Server nie jest zaimplementowane;
podanie `--tls-cert`/`--tls-key` dla tego silnika kończy się błędem `DBP1015E`,
zamiast cichego ignorowania plików.

## Pominięcie weryfikacji

`--tls-skip-verify` służy wyłącznie do diagnostyki. Nie używaj go podczas zbierania Blueprint produkcyjnej bazy danych, chyba że zespół bezpieczeństwa wyraźnie to zatwierdził.

## Dziennik audytu

Dziennik audytu zapisuje żądany tryb TLS, ścieżki CA i certyfikatu oraz informację o pominięciu weryfikacji. Po udanym połączeniu zapisuje, czy TLS wynegocjowano; obecne sterowniki nie udostępniają wiarygodnej wersji protokołu, więc jest ona oznaczona jako niedostępna. Klucze prywatne nie są zapisywane.

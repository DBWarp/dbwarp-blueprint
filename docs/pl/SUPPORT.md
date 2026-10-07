# Wsparcie i zgłaszanie problemów

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i może zawierać błędy. Nie powinna być traktowana jako tekst kontraktowy. Zobacz [kanoniczne źródło angielskie](../../SUPPORT.md).

**Języki:** [English](../../SUPPORT.md) | [Deutsch](../de/SUPPORT.md) | [Français](../fr/SUPPORT.md) | [Español](../es/SUPPORT.md) | **Polski** | [日本語](../ja/SUPPORT.md) | [简体中文](../zh/SUPPORT.md)

Korzystaj z [systemu zgłoszeń](https://github.com/DBWarp/dbwarp-blueprint/issues)
w przypadku pytań instalacyjnych niezawierających danych wrażliwych,
odtwarzalnych błędów i propozycji funkcji. Ten kanał nie gwarantuje czasu
odpowiedzi ani poziomu świadczenia usług wsparcia.

Podejrzewane podatności zgłaszaj prywatnie drogą opisaną w
[SECURITY.md](SECURITY.md), a nie w publicznym zgłoszeniu.

## Pomocne informacje

- Dokładny tag wydania, suma kontrolna pliku binarnego, system operacyjny i
  architektura.
- Silnik i wersja bazy danych lub format pliku strukturalnego oraz informacja,
  czy źródło jest zarządzane samodzielnie, czy w ramach usługi zarządzanej.
- Opcje polecenia z usuniętymi poświadczeniami, punktami końcowymi, ścieżkami i
  identyfikującymi selektorami albo z ich zamiennikami wyraźnie oznaczonymi jako
  przykłady.
- Kod diagnostyczny `DBP`, oczekiwane zachowanie i rzeczywiste zachowanie.
- Niewielkie syntetyczne odtworzenie, jeśli to możliwe.

Nie przesyłaj danych produkcyjnych, poświadczeń ani nieprzejrzanego audytu,
pakietu, Blueprint lub prezentacji. Audyty mogą zawierać tożsamości i punkty
końcowe; anonimowe pliki Blueprint mogą nadal ujawniać charakterystyczną
strukturę obciążenia. Zacznij od minimalnego bezpiecznego opisu i przejrzyj każdy
załącznik przed jego udostępnieniem.

## Obsługiwane konfiguracje

[STATUS.md](../../STATUS.md) opisuje możliwości oraz obsługiwane wersje baz danych. [BUILD.md](BUILD.md) opisuje wymagania dotyczące kompilacji, specyficzne dla platformy i uwierzytelniania. Wersja języka Rust jest zablokowana do konkretnej wersji w `rust-toolchain.toml`; inne środowiska programistyczne niekoniecznie zostały przetestowane.

Wytyczne dotyczące uprawnień dla usług zarządzanych nie oznaczają, że każda usługa lub konfiguracja została przetestowana. Należy użyć odpowiednich [wymagań dotyczących uprawnień](../../sql/grants/DATABASE_PERMISSIONS.md) i przetestować konkretną konfigurację przed użyciem w środowisku produkcyjnym.

Zmiany między wersjami kolektora opisuje [CHANGELOG.md](CHANGELOG.md).

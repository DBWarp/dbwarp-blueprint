# Internacjonalizacja

> **Tłumaczenie wspomagane maszynowo:** ta wersja oczekuje na weryfikację techniczną przez osobę biegle posługującą się językiem polskim i nie powinna być traktowana jako tekst kontraktowy. [Kanoniczne źródło angielskie](../INTERNATIONALISATION.md).

[English](../INTERNATIONALISATION.md) | [Deutsch](../de/INTERNATIONALISATION.md) | [Français](../fr/INTERNATIONALISATION.md) | [Español](../es/INTERNATIONALISATION.md) | [Polski](INTERNATIONALISATION.md) | [日本語](../ja/INTERNATIONALISATION.md) | [简体中文](../zh/INTERNATIONALISATION.md)

`dbwarp-blueprint` oddziela prezentację dla człowieka od składni operacyjnej. Jest
to granica bezpieczeństwa i automatyzacji, a nie tylko preferencja wyświetlania.

## Obsługiwane języki

Angielski tekst źródłowy jest wiążący. Katalogi prezentacji w innych językach
są wspomagane maszynowo i mogą zawierać błędy, mimo że ich pokrycie kluczy i
tokenów jest sprawdzane. Decyzje dotyczące bezpieczeństwa, umów, przepisów i
najmniejszych uprawnień porównuj z tekstem angielskim. Osobną bramkę publikacji
przetłumaczonych dokumentów opisuje [`TRANSLATIONS.md`](../TRANSLATIONS.md).

| Wartość | Język | Tag ustawień regionalnych używany w generowanych prezentacjach |
|---|---|---|
| `en` | angielski | `en-US` |
| `de` | niemiecki | `de-DE` |
| `fr` | francuski | `fr-FR` |
| `es` | hiszpański | `es-ES` |
| `pl` | polski | `pl-PL` |
| `ja` | japoński | `ja-JP` |
| `zh` | chiński uproszczony | `zh-CN` |

Jawnie wybierz język:

```bash
dbwarp-blueprint --lang de --help
dbwarp-blueprint --lang ja --connect postgresql://db.internal/app --dry-run
```

Jeżeli `--lang` nie występuje, kolejność rozstrzygania jest następująca:

1. `DBWARP_BLUEPRINT_LANG`;
2. `LC_ALL`;
3. `LC_MESSAGES`;
4. `LANG`;
5. język angielski.

Dla tagów ustawień regionalnych środowiska akceptowane są sufiksy regionu i
kodowania, dlatego `de_CH.UTF-8`, `pl_PL.UTF-8` i `ja-JP` są rozstrzygane do
języka bazowego. Jawne wartości `--lang` są celowo ograniczone do siedmiu
kanonicznych tokenów w tabeli.

W Windows zmienne `LC_ALL`, `LC_MESSAGES` i `LANG` zwykle nie są ustawione, więc narzędzie używa angielskiego, chyba że podano `--lang` lub `DBWARP_BLUEPRINT_LANG`, na przykład `$env:DBWARP_BLUEPRINT_LANG = "de"` w PowerShell albo `set DBWARP_BLUEPRINT_LANG=de` w cmd. Nazwy zmiennych w Windows nie rozróżniają wielkości liter, a w Linux i macOS rozróżniają; zawsze używaj kanonicznych wielkich liter.

## Co jest tłumaczone

- opisy pomocy najwyższego poziomu i opcji;
- elementy konstrukcyjne pomocy, takie jak etykiety użycia i możliwych wartości;
- plany kontroli wstępnej i monity o zgodę;
- podsumowanie, przyczyna i działanie naprawcze komunikatu DBP;
- treść postępu i ostrzeżeń;
- nagłówki, etykiety, objaśnienia i metadane ustawień regionalnych w prezentacjach PowerPoint.

Krytyczne szczegóły techniczne mogą pozostać bez zmian pod zlokalizowanym komunikatem DBP, gdy są potrzebne do diagnozy. Ostrzeżenia niekrytyczne ukrywają surowe szczegóły sterownika, jeśli mogłyby zawierać identyfikatory źródła; stabilny kod DBP i anonimowy cel Blueprint pozostają dostępne.

## Co nigdy się nie zmienia

Poniższe elementy pozostają kanonicznymi tokenami angielskimi w każdym języku prezentacji:

- polecenie `dbwarp-blueprint` i nazwy opcji, takie jak `--measure-compression`;
- akceptowane wartości, takie jak `verify-full`, `balanced` i `exact`;
- schematy URI, takie jak `postgresql://`, `mysql://` i `sqlserver://`;
- nazwy zmiennych środowiskowych i ścieżki plików;
- selektory, takie jak `source=ID` i `table=ID`;
- identyfikatory DBP, takie jak `DBP1001E`;
- zanonimizowane identyfikatory, takie jak `table-001`, `col-1` i `schema-A`;
- klucze audytu, klucze TOML, klucze pakietów, nazwy typów baz danych i metody indeksowania.

Dzięki temu skrypty nie wymagają obsługi opcji ani wartości zależnej od języka,
a Blueprint wygenerowany z `--lang ja` jest identyczny bajt w bajt z Blueprint
wygenerowanym z `--lang en`, gdy wszystkie pozostałe deterministyczne dane
wejściowe są takie same.

## Rygorystyczne zachowanie katalogu

Wszystkie katalogi są kompilowane w pliku binarnym. Podczas uruchamiania program
sprawdza, czy każde reklamowane ustawienie regionalne inne niż angielskie
dokładnie obejmuje:

- bieżące drzewo pomocy Clap na żywo;
- każdy stabilny kod DBP i wszystkie trzy pola diagnostyczne;
- każdy stabilny monit, komunikat postępu, ostrzeżenie i klucz prezentacji;
- każdy wymagany symbol zastępczy i chroniony token operacyjny.

Brakujące lub nadmiarowe wpisy, zmiany symboli zastępczych, zmienione tokeny
operacyjne, nieprawidłowy JSON albo niewidoczne/dwukierunkowe znaki formatujące
powodują zamknięcie z błędem `DBP1010E`. Program nie zastępuje po cichu
brakującego tłumaczenia językiem angielskim.

## Przepływ pracy opiekuna

Kanonicznym źródłem jest angielska pomoc Rust oraz definicje komunikatów/UI
w `src/i18n.rs`. Gdy zmienia się dowolny tekst widoczny dla klienta:

1. zaktualizuj w tym samym commicie każdy katalog językowy w `locales/`;
2. zachowaj dokładnie wszystkie symbole zastępcze i kanoniczne tokeny operacyjne;
3. uruchom ukierunkowany test dokładnego pokrycia;
4. dodaj lub zaktualizuj odpowiedni przypadek granicy operatorskiej w
   `tests/cli_errors.rs`, gdy zmienia się awaria lub ostrzeżenie;
5. uruchom pełny zestaw testów i sprawdź reprezentatywne dane wyjściowe pomocy/prezentacji;
6. uzyskaj weryfikację techniczną przez osobę biegle posługującą się danym językiem, zanim nowe brzmienie zostanie uznane za ostateczne dla umowy z klientem, zgłoszenia regulacyjnego lub publicznego materiału marketingowego.

Ukierunkowana walidacja:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

Testy integracyjne dowodzą również, że tokeny opcji są identyczne we wszystkich
językach, zlokalizowane kody DBP pozostają stabilne, emitowany TOML nie zależy od
języka, a treść wygenerowanej prezentacji zawiera wybrane ustawienia regionalne.

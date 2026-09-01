# Internationalisation

`dbwarp-blueprint` separates human presentation from operational syntax. This is a
security and automation boundary, not merely a display preference.

## Supported Languages

English source text is authoritative. The non-English presentation catalogs
are machine-assisted and may contain errors even though their key/token
coverage is validated. Compare security, contractual, regulatory, and
least-privilege decisions with the English text. See
[`TRANSLATIONS.md`](TRANSLATIONS.md) for the separate, non-blocking
translated-document maintenance cycle.

| Value | Language | Locale tag used in generated decks |
|---|---|---|
| `en` | English | `en-US` |
| `de` | German | `de-DE` |
| `fr` | French | `fr-FR` |
| `es` | Spanish | `es-ES` |
| `pl` | Polish | `pl-PL` |
| `ja` | Japanese | `ja-JP` |
| `zh` | Simplified Chinese | `zh-CN` |

Select a language explicitly:

```bash
dbwarp-blueprint --lang de --help
dbwarp-blueprint --lang ja --connect postgresql://db.internal/app --dry-run
```

When `--lang` is absent, resolution order is:

1. `DBWARP_BLUEPRINT_LANG`;
2. `LC_ALL`;
3. `LC_MESSAGES`;
4. `LANG`;
5. English.

Region and encoding suffixes are accepted for environment locale tags, so
`de_CH.UTF-8`, `pl_PL.UTF-8`, and `ja-JP` resolve to their base language.
Explicit `--lang` values are deliberately limited to the seven canonical
tokens in the table.

On Windows, `LC_ALL`, `LC_MESSAGES`, and `LANG` are normally not set, so the
tool runs in English unless `--lang` is given or `DBWARP_BLUEPRINT_LANG` is
set, for example `$env:DBWARP_BLUEPRINT_LANG = "de"` in PowerShell or
`set DBWARP_BLUEPRINT_LANG=de` in cmd. Environment variable names are
case-insensitive on Windows and case-sensitive on Linux and macOS; always use
the canonical upper-case names.

## What Is Translated

- top-level and option help descriptions;
- help scaffolding such as usage and possible-values labels;
- pre-flight plans and consent prompts;
- DBP message summary, cause, and corrective action;
- progress and warning prose;
- PowerPoint deck headings, labels, explanations, and locale metadata.

Fatal low-level details may remain verbatim beneath the localized DBP message
when they are required for diagnosis. Non-fatal database warnings redact raw
driver detail when it could contain source identifiers; the stable DBP code and
anonymous Blueprint target remain available for triage.

## What Never Changes

The following remain canonical English tokens in every presentation language:

- `dbwarp-blueprint` command and option names such as `--measure-compression`;
- accepted values such as `verify-full`, `balanced`, and `exact`;
- URI schemes such as `postgresql://`, `mysql://`, and `sqlserver://`;
- environment-variable names and file paths;
- selectors such as `source=ID` and `table=ID`;
- DBP identifiers such as `DBP1001E`;
- anonymized identifiers such as `table-001`, `col-1`, and `schema-A`;
- audit keys, TOML keys, bundle keys, database type names, and index methods.

Consequently, scripts do not need language-specific option or value handling,
and language choice does not alter Blueprint fields. A byte-identical
cross-language comparison additionally requires the same source state,
options, pinned `--generated-at`, producer, and protected
`--anonymization-key-file`; separate default runs intentionally use different
keys.

## Strict Catalog Behavior

All catalogs are compiled into the binary. At startup the program verifies
that every advertised non-English locale exactly covers:

- the current live Clap help tree;
- every stable DBP code and all three diagnostic fields;
- every stable prompt, progress, warning, and deck key;
- every required placeholder and protected operational token.

Missing or extra entries, placeholder changes, altered operational tokens,
invalid JSON, or invisible/bidirectional format controls fail closed with
`DBP1010E`. The program does not silently substitute English for a missing
translation.

## Maintainer Workflow

The canonical source is the English Rust help and the message/UI definitions
in `src/i18n.rs`. When any customer-visible phrase changes:

1. update every locale catalog under `locales/` in the same commit;
2. retain all placeholders and canonical operational tokens exactly;
3. run the focused exact-coverage test;
4. add or update the relevant operator-boundary case in
   `tests/cli_errors.rs` when a failure or warning changes;
5. run the full test suite and inspect representative help/deck output;
6. obtain native technical review before treating new wording as final for a
   customer contract, regulatory filing, or public marketing material.

This exact-coverage workflow applies to runtime catalogs embedded in the
binary. Translated Markdown is supplemental and follows the non-blocking cycle
described in [`TRANSLATIONS.md`](TRANSLATIONS.md); English documentation alone
is release-blocking.

Focused validation:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

The integration tests also prove that option tokens are identical across
languages, localized DBP codes stay stable, emitted TOML is language-invariant,
and generated deck prose carries the selected locale.

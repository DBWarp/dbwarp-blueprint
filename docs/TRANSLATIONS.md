# Documentation Translations

English documentation in the repository root and `docs/` is the authoritative
DBWarp Blueprint operating and security contract. Release qualification checks
the English documentation. A stale or incomplete translated-document set does
not block an otherwise qualified release.

The public source and release archives may also include machine-translated
customer documents for German (`de`), French (`fr`), Spanish (`es`), Polish
(`pl`), Japanese (`ja`), and Simplified Chinese (`zh`). These documents are
supplemental. They may lag the current English source and may contain
mistranslations, awkward wording, or technical errors. When wording differs,
the English source wins.

Do not rely on a translation for a security, regulatory, contractual, or
least-privilege decision without checking the English source. See
[Machine-translated material](../MACHINE_TRANSLATIONS.md).

Runtime help, prompts, diagnostics, progress messages, and deck prose use a
separate embedded-catalog contract. Every language advertised by the binary
must still cover the live CLI, DBP messages, and stable presentation keys. The
normal Rust test suite enforces that runtime contract.

## Protected Operational Syntax

Translations must not change command names, option names, accepted values, URI
schemes, environment-variable names, selectors, DBP codes, audit or TOML keys,
database identifiers, file paths, URLs, SQL, or fenced code blocks. These are
automation and support tokens and remain canonical English in every language.

## Translation Maintenance

Internal translation-maintenance records and validation tooling identify which
translations differ from a recorded English revision and verify protected
syntax, links, and locale-set integrity. Maintainers run those checks when
preparing or claiming a synchronized translated-document set; they are not
part of the public release package.

A failing translation check means the translated set must not be described as
current with English. It does not invalidate the English release candidate.
Updating a manifest hash acknowledges completed translation review; it must
not be used to hide untranslated or unreviewed changes.

Multiple independent machine reviews can improve a translation, but they are
not native-language, legal, regulatory, or contractual approval. Obtain native
technical or legal review when translated wording will be relied upon for
those purposes.

# Documentation Translations

English documentation in the repository root and `docs/` is the authoritative
DBWarp Blueprint operating and security contract.

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
binary validates that runtime coverage at startup.

## Protected Operational Syntax

Translations must not change command names, option names, accepted values, URI
schemes, environment-variable names, selectors, DBP codes, audit or TOML keys,
database identifiers, file paths, URLs, SQL, or fenced code blocks. These are
automation and support tokens and remain canonical English in every language.

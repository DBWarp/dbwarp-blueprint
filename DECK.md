# Visual summary deck

`dbwarp-blueprint --deck blueprint.pptx` writes an optional PowerPoint (`.pptx`) summary
of the Blueprint alongside the `--out` TOML file. `dbwarp-blueprint --from-toml
blueprint.toml --deck blueprint.pptx` builds the same deck later from an existing
reviewed Blueprint file, without connecting to a database. It is a presentation of
the same anonymized data: nothing more is read from or sent to your database.
The deck computes only the documented local summaries and projections from
fields already present in the Blueprint.

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

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx \
  --lang ja
```

`--lang en|de|fr|es|pl|ja|zh` localizes the deck's human-facing prose and
PowerPoint language metadata. Anonymous identifiers, database type names,
index methods, measurements, and the source TOML remain canonical and
language-neutral. Catalog validation fails closed rather than substituting
English for a missing deck phrase. See
[`docs/INTERNATIONALISATION.md`](docs/INTERNATIONALISATION.md).

## Footer and confidentiality

Every content slide follows the DBWarp house footer: a small lockup at left, an
optional separator and confidentiality level, a bare centred slide number, and
`DBWarp.com` at right. The title slide remains unnumbered.

Use `--deck-confidentiality public|internal|confidential|restricted` to add one
of the localized built-in classification labels. Any other safe, non-empty
value is a custom label and is displayed verbatim; quote values containing
spaces, for example `--deck-confidentiality "CLIENT // SENSITIVE"`. Labels
cannot have leading or trailing whitespace, control or bidirectional formatting
characters, or exceed 48 display-width units. Omit the option for no
classification label. The setting changes only the deck presentation; it does
not alter the Blueprint file or the data summarised by the deck. Given the
exact same reviewed Blueprint, language, label, and timestamp, the deck bytes
are reproducible.

## Trust properties

- **Built locally, from memory.** The deck is rendered from the same in-memory
  Blueprint that produces `blueprint.toml`. There is no extra database query and no
  second pass over the catalog. In `--from-toml` mode, the in-memory Blueprint is
  loaded from the reviewed TOML file instead.
- **No application network.** Generating the deck opens no network connection;
  a Blueprint read from a network-mounted path remains subject to the host
  storage stack.
- **No third-party library.** The OOXML is authored directly in `src/deck.rs`;
  the `.pptx` is a plain ZIP of XML parts you can `unzip` and read. No PowerPoint
  automation, no rendering service, no extra crate in the dependency graph. The
  approved DBWarp logo images and static DM Sans font faces are embedded in the
  Rust binary and written as OOXML media/font parts; generation does not read a
  runtime asset path.
- **No real identifiers, no row data.** Tables, columns, and indexes appear as
  the same anonymous placeholders as the Blueprint file (`table-001`, `col-1`,
  `idx-1`, `schema-A`). Source measurements retain their documented precision;
  any projection is computed only from fields already in the Blueprint. The
  deck contains no customer-specific information beyond that input.
- **Reproducible from fixed input.** The exact same reviewed Blueprint produces
  a byte-identical `.pptx` for the same selected language, confidentiality
  label, and pinned timestamp (fixed part order and timestamps). This does not
  make two live captures identical: those also require the same protected
  `--anonymization-key-file`, source state, and capture options.

## What it contains

The deck adapts to schema size:

- **Title** — DBWarp logo and tagline, engine, version, source kind, table
  count, generation timestamp.
- **Executive summary** — management-facing migration scale, data
  concentration, relationship complexity, and review evidence signals.
- **Overview** — table / row / data-size / index-size totals, plus column,
  index, foreign-key, and schema counts.
- **Small schemas** (a few tables) — a sized panel per table (rows, bytes,
  column types, indexes) and a foreign-key diagram.
- **Large schemas** — characterization instead of enumeration:
  - *Largest tables*: the top tables by size, with a `+ N more` remainder.
  - *Schema composition*: column-type distribution and index/total statistics.
  - *Relationships*: foreign-key count, connected vs standalone tables, and the
    most-referenced (hub) tables.
- **Measured compression** (Tier 2 only) — sampled-table count, weighted zstd-3
  ratio, projected compressed footprint, and the most-compressible sampled
  tables.
- **Trust model** — a closing slide summarising the guarantees above.

## Reviewing the output

The `.pptx` is a standard OOXML package. To audit exactly what it contains:

```bash
unzip -l blueprint.pptx           # list parts
unzip -p blueprint.pptx ppt/slides/slide1.xml   # read a slide as plain XML
```

Open it in PowerPoint, LibreOffice Impress, or Google Slides. The generator is
[`src/deck.rs`](src/deck.rs) and is built into the Rust binary. There is no
separate deck generator to install, audit, or keep in sync.

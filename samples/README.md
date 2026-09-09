# Reviewable Blueprint examples

These small schema-v6 files are **hand-authored synthetic illustrations**, not
customer captures, performance results or evidence that a particular deployment
has been qualified. Numbers and inventory entries are invented to explain the
contract. Normal captures vary with permissions, selected schemas, database
state and sampling options. Omitted optional fields are not proof of absence.

| Example | What to inspect |
|---|---|
| [PostgreSQL catalog-only](postgresql-v6-catalog.toml) | Table/index structure, statistical row counts, stale statistics, selected scope, unknown topology and a privilege-filtered artifact summary. No row-sampling blocks. |
| [MySQL sampled](mysql-v6-sampled.toml) | Exact declared lengths, observed length/style/null/cardinality aggregates, sample bias, table and column compression tags, and network timing. Statistics freshness is deliberately absent. |
| [SQL Server sampled and analyzed](sqlserver-v6-analyzed.toml) | Foreign keys, Unicode lengths, aggregate distributions, partial replicated topology, object dependencies, lexical feature bands, unresolved references and an external assembly whose code is not captured. |

All three declare `source_kind = "synthetic"`. Their engine versions are
illustrative values, not the support matrix. Completeness refers to the
declared dataset scope: `selection-limited` is not an assertion about every
schema or member of a deployment. The analyzed example deliberately shows
incomplete evidence rather than implying that every optional query succeeded.

For an offline first run, from the directory containing the binary and samples:

```bash
./dbwarp-blueprint --from-toml samples/sqlserver-v6-analyzed.toml --deck sample.pptx
```

This renders the supplied metadata; it does not connect to a database or
re-measure the sample. `measured = true` describes the kind of measurement
being illustrated, not a measurement made while loading this example.

The files expose aggregate information, not row values. That still includes
declared lengths, source collation metadata, distributions, relationship and
object counts, and coarse feature bands. A measured `ratio_stddev = 0.0` is
different from a missing measurement. A small `sample_bytes` value can round
to zero under the documented bucket policy. Review [FORMAT.md](../FORMAT.md)
and [the DBA guide](../docs/DBA_REVIEW_GUIDE.md) for the complete output surface,
precision rules, permissions and consent; these examples are not exhaustive.

The larger `saas-medium.toml`, `ecommerce-large.toml` and `erp-enterprise.toml`
files remain **legacy schema-v1 compatibility fixtures**. They can still be
rendered, but omit much of today's disclosure surface and must not be used as
the sole basis for approving a current capture.

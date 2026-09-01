# Cookbook

Task-oriented recipes for common `dbwarp-blueprint` workflows.

## Recipe: Localized Operator Session

Select one of the complete embedded language catalogs while keeping commands,
values, identifiers, and output schemas canonical:

```bash
./dbwarp-blueprint --lang de --help
./dbwarp-blueprint --lang ja \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail none \
  --tls-mode verify-full --tls-ca /etc/pki/internal-root.crt \
  --out pg-appdb.blueprint.toml --yes
```

For unattended runs, set `DBWARP_BLUEPRINT_LANG=fr` or a standard process locale.
An explicit `--lang` always wins. DBP codes and low-level provider details stay
canonical so a localized failure can be searched and shared with support.

## Recipe: PostgreSQL With Internal CA

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out pg-appdb.blueprint.toml \
  --audit-log pg-appdb.audit.txt
```

Use this for normal production PostgreSQL review. If hostname verification fails, fix the server certificate or use the correct DNS name; do not use `--tls-skip-verify` except for loopback tests.

## Recipe: MySQL With Username File

Useful when the username contains characters awkward to URI-encode.

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

For a performance-representative synthetic reconstruction, use the default
balanced policy: exact MySQL declaration/index metadata and tightly rounded
sampled widths:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --user-file /etc/dbwarp/mysql-blueprint.user \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/mysql-ca.pem \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml \
  --audit-log mysql-appdb.audit.txt
```

Confirm `declared_length_fidelity = "exact"`,
`index_length_fidelity = "exact"`, and
`observed_length_fidelity = "relative-rounded-v2"`. Use
`--length-fidelity exact --yes` only after the customer approves sharing exact
sampled length statistics. Names and values remain excluded.

On estates with thousands of tables, raise `--max-wall-secs` above its 300-second
default if needed. Fidelity markers certify policy, while the downstream
estimator separately requires observed average/p95 lengths for every nonempty
variable-width indexed column before marking a fixture benchmark-ready.

## Recipe: SQL Server SQL Authentication

```bash
./dbwarp-blueprint \
  --connect sqlserver://sql-blueprint@sql-primary.internal,1433/appdb \
  --password-file /etc/dbwarp/sql-blueprint.pass \
  --auth-mode sql-auth \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-appdb.blueprint.toml \
  --audit-log mssql-appdb.audit.txt
```

SQL Server certificate-verifying TLS modes use the operating-system trust store
when `--tls-ca` is omitted. A supplied `.pem` or `.crt` file must contain
exactly one CA certificate and replaces those roots. Both `verify-ca` and
`verify-full` validate the connection hostname.

## Recipe: SQL Server Entra ID Token

Generate the token outside the tool, then pass it by file:

```bash
install -d -m 700 "$HOME/.cache/dbwarp-blueprint"
TOKEN_FILE="$HOME/.cache/dbwarp-blueprint/sql-token"
az account get-access-token \
  --resource https://database.windows.net/ \
  --query accessToken -o tsv > "$TOKEN_FILE"
chmod 600 "$TOKEN_FILE"

./dbwarp-blueprint \
  --connect sqlserver://sql-primary.database.windows.net,1433/appdb \
  --user sql-blueprint@tenant.example \
  --auth-mode entra-token \
  --azure-token-file "$TOKEN_FILE" \
  --tls-mode verify-full \
  --tls-ca /etc/pki/sqlserver-ca.pem \
  --measure-compression --yes \
  --out mssql-entra.blueprint.toml \
  --audit-log mssql-entra.audit.txt
```

## Recipe: Catalog-Only Security Review

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out catalog-only.blueprint.toml \
  --audit-log catalog-only.audit.txt \
  --yes
```

This is the lowest-friction review mode. It avoids row sampling but produces less accurate compression and egress estimates downstream.

## Recipe: Assess Non-Table Migration Complexity

Start with the default summary to collect counts and external prerequisites
without reading definitions:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail summary \
  --out appdb-summary.blueprint.toml \
  --audit-log appdb-summary.audit.txt \
  --yes
```

After security approval, collect anonymous dependencies and bounded language
complexity evidence:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --artifact-detail analyzed \
  --out appdb-analyzed.blueprint.toml \
  --audit-log appdb-analyzed.audit.txt \
  --yes
```

Review `visibility`, all three completeness flags, `catalogs_unreadable`,
`families_not_inventoried`, and `counts_by_external_class`. Treat each external
class as an explicit migration task. Do not treat an inventoried object as proof
that DBWarp can recreate or translate it; compare it with the migration
capability matrix. See [`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Recipe: Disable the RTT Probe

By default, the tool runs five `SELECT 1` probes after connection setup and emits a `[network]` block. If a DBA forbids non-catalog queries, disable it:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal:5432/appdb \
  --password-file /etc/dbwarp/pg-blueprint.pass \
  --no-rtt-probe \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

The RTT probe never reads row data; each query returns the constant integer `1`.

## Recipe: Time-Box Compression Sampling

For large production systems, keep the first run conservative:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal/appdb \
  --password-file /etc/dbwarp/mysql.pass \
  --measure-compression --yes \
  --sample-rows 500 \
  --max-wall-secs 120 \
  --out blueprint.toml \
  --audit-log audit.txt
```

If the output marks many samples as biased or missing, rerun from a read replica with a larger time budget.

## Recipe: One Customer, Multiple Databases

Use a batch manifest when a customer wants one reviewed package for several databases.

`customer.batch.toml`:

```toml
[defaults]
measure_compression = true
sample_rows = 1000
max_wall_secs = 300
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["erp", "critical"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
tags = ["billing"]

[[source]]
id = "warehouse_sql"
kind = "sqlserver"
connect_env = "WAREHOUSE_SQL_URI"
password_file = "/etc/dbwarp/warehouse.pass"
auth_mode = "sql-auth"
tags = ["warehouse"]
```

Dry-run:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Run:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

This writes `bundle.toml`, one child Blueprint per source, and one audit per source.
The child Blueprints remain reviewable independently.

## Recipe: One Customer, Mixed Databases And Data Lake Files

Use structured-file sources in the same batch when the customer has Parquet or Avro extracts next to live databases.

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
tags = ["database"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/mnt/customer/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/mnt/customer/events/*.avro"]
dataset_mode = "one_table_per_file"
tags = ["lake", "events"]
```

`partitioned_dataset` currently merges files like `merge_same_schema`, but keeps the customer's intent visible in the bundle. Keep unrelated schemas in separate sources.

## Recipe: Extract Only One Source Or Table From A Bundle

After a batch run, list sources:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

Extract one source:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Extract one table from one source:

```bash
./dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Use this when the customer approves only part of an estate for a benchmark, or when you want to generate a small focused fixture from a large bundle.

## Recipe: Pack A Separately Reviewed Bundle For Handoff

The working bundle directory contains child Blueprints and access-controlled
audits. Do not transfer it wholesale. After reviewing the manifest values and
child Blueprints, create a single-file handoff:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

The packed file retains operator-supplied source IDs, tags, dataset-group IDs,
and audit-path metadata. Use anonymous values, inspect the packed TOML, and
transfer it only through the approved channel.

## Recipe: Batch Handoff Package

Create a directory like this:

```text
customer-blueprint-handoff/
  customer-blueprint-bundle.packed.toml
  customer.batch.toml.redacted
  reviewer-notes.md       # optional
```

Build this separate directory from reviewed copies. Keep the working
`bundle.toml`, `blueprints/`, `audits/`, and any `errors.txt` local and
access-controlled. `customer.batch.toml.redacted` should show only approved
source IDs, kinds, tags, and dataset modes. Do not include secrets, private
hostnames, password files, token files, private keys, database logs, or decoded
row samples.

## Recipe: Offline Deck From Reviewed TOML

```bash
./dbwarp-blueprint \
  --from-toml reviewed.blueprint.toml \
  --deck reviewed.blueprint.pptx
```

This mode reads only the TOML file and writes the deck. It rejects live database flags instead of silently ignoring them.

## Recipe: Byte-Identical Reproducibility

Pin the timestamp and reuse the same protected customer-held anonymization key:

```bash
./dbwarp-blueprint \
  --connect postgresql://pg-blueprint@pg-primary.internal/appdb \
  --password-file /etc/dbwarp/pg.pass \
  --anonymization-key-file /etc/dbwarp/anonymization.key \
  --generated-at "2026-04-26T00:00:00Z" \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

The key file must contain exactly 32 raw bytes or 64 hexadecimal characters,
must not be group/world-readable on Unix, and must never be included in the
handoff. Without this option, a fresh operating-system-random key intentionally
changes anonymous label ordering on every run. Pinning only `--generated-at`
is insufficient. Use the complete recipe for approved forensic snapshots; a
deck generated twice from the exact same reviewed Blueprint remains
byte-identical when its timestamp and language are unchanged.

## Recipe: Handoff Package for DBWarp

Create a directory like this:

```text
customer-blueprint-handoff/
  blueprint.toml
  blueprint.pptx              # optional
  command-used.redacted.txt
  reviewer-notes.md           # optional
```

`command-used.redacted.txt` may record the approved flags and sampling budgets,
but remove credentials, tokens, private hostnames, and local paths. Keep
`audit.txt` locally as access-controlled operational evidence. Include it only
for a named support need through an approved secure channel. Do not include
password files, token files, private keys, or database logs.

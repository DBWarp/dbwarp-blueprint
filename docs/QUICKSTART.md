# Quickstart

This quickstart is for a DBA or security reviewer who needs to produce a shareable DBWarp Blueprint file without exposing data.

## 1. Choose How to Run the Tool

Use one of these paths:

- Download a release binary and verify its checksum.
- Build from source with `./build.sh`.
- Build from the vendored release bundle for strict offline dependency review.

See [`../BUILD.md`](../BUILD.md) and [`../binaries/README.md`](../binaries/README.md).

Select a presentation language explicitly when required:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --schema app --dry-run
```

Supported values are `en`, `de`, `fr`, `es`, `pl`, `ja`, and `zh`. The
presentation language changes help, prompts, diagnostics, progress text, and
deck prose. It never changes option names, accepted values, URI schemes,
selectors, DBP codes, audit keys, or Blueprint TOML. See
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Provision a Dedicated Least-Privilege Account

Do this before any live connection, including `--dry-run` examples that will
later be turned into captures. Do not begin with an application owner,
administrator, superuser, `root`, `sa`, or `db_owner` account.

1. Identify the exact engine/version, database, and approved schema or schemas.
2. Choose the capture tier: `basic` for table catalogs only, `standard` to add a
   bounded row sample, or `enhanced` for non-table object
   analysis as well.
3. Have the DBA copy the matching script under `sql/grants/<engine>/`, edit all
   marked database, schema, principal, password, and role-toggle values, and
   run it through the normal change-control process.
4. Use the dedicated account it creates and pass the same approved scope with
   one `--schema NAME` option per schema on every live command.
5. After the capture has been reviewed, have the DBA review and run the
   matching engine script under `sql/revoke/` to remove the account and grants.

The scripts deliberately distinguish literal scoped grants from convenient
built-in roles and explain where a role is broader. Read
[`../sql/grants/README.md`](../sql/grants/README.md) for the runnable scripts
and [`../sql/grants/DATABASE_PERMISSIONS.md`](../sql/grants/DATABASE_PERMISSIONS.md)
for the version-aware DBA/security rationale. The collector does not create,
broaden, or remove database principals itself.

## 3. Prepare Credentials Safely

Do not put passwords in the connection URI. The tool refuses URI-embedded passwords to avoid process-list and shell-history leaks.

Preferred password-file pattern (the secret is entered without echo and does
not appear in shell history):

```bash
sudo install -d -m 700 -o "$USER" -g "$(id -gn)" /etc/dbwarp
install -m 600 /dev/null /etc/dbwarp/db.pass
read -rsp 'Database password: ' DBWARP_BP_PASSWORD; printf '\n'
printf '%s' "$DBWARP_BP_PASSWORD" > /etc/dbwarp/db.pass
unset DBWARP_BP_PASSWORD
```

If the username is awkward to URI-encode, place it in a file too:

```bash
install -m 600 /dev/null /etc/dbwarp/db.user
printf '%s' 'DOMAIN\migration_user' > /etc/dbwarp/db.user
```

Then use `--user-file /etc/dbwarp/db.user`.

## 4. Dry-Run First

A dry run validates arguments and prints the planned action without connecting:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

For `--from-toml` deck mode, dry-run is a local preflight and does not read the database.

For multiple sources, dry-run the batch manifest instead:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 5. Run Catalog-Only Mode

This strict catalog-only mode reads table metadata and statistics, but no row
samples or non-table object catalogs:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.catalog.toml \
  --audit-log blueprint.catalog.audit.txt \
  --yes
```

Use this when a policy forbids row sampling or when you want a first security-review pass.

## 6. Choose Non-Table Artifact Detail

The default `--artifact-detail summary` reads non-table catalogs but not object
definitions. It emits bounded counts and external-prerequisite classes. Use
`--artifact-detail none` if policy forbids those catalogs. The count-only
topology probe still runs; see the
[grant reference](../sql/grants/README.md#topology-evidence).

For anonymous dependency topology, use `graph`. For bounded language-feature
and complexity bands, use `analyzed`. Both require explicit consent:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.analyzed.toml \
  --audit-log blueprint.analyzed.audit.txt \
  --yes
```

The output never contains object names, definition text, endpoints, secrets,
keys, certificates, or binaries. See
[`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md) before approving graph or
analyzed mode.

## 7. Run Tier 2 Compression Measurement

Tier 2 reads bounded row samples into memory, computes aggregate compression,
null-density, cardinality/frequency, length, and style measurements, and
discards the sampled values:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

Use Tier 2 when possible. It gives more accurate transfer-size and egress-cost estimates.

## 8. Generate a Deck

During the live run:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --schema app \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --audit-log blueprint.audit.txt \
  --yes
```

Or after review, with no database connection:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

## 9. Review Before Sharing

Review:

```bash
less blueprint.toml
less blueprint.audit.txt
unzip -l blueprint.pptx  # optional deck package inspection
```

Expected properties:

- no real table names;
- no real column names;
- no row values;
- no comments except the fixed header;
- rounded counts and byte sizes;
- anonymized ids such as `table-001`, `col-1`, and `schema-A`;
- bounded artifact counts and, when approved, anonymous artifact ids;
- explicit incomplete/unreadable artifact evidence rather than silent omission;
- optional aggregate compression, null-density, cardinality/frequency, length,
  and style measurements, never sampled values.

## 10. Share with DBWarp

Minimum to share:

```text
blueprint.toml
```

For multiple sources, create and inspect a packed bundle rather
than sharing the working directory:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Bundle metadata retains the source ids, tags, and dataset-group ids chosen in
the batch manifest. Use anonymous values and review them before transfer.

See [Batch Collection And Blueprint Bundles](BATCH_AND_BUNDLES.md) if you have several databases or several Parquet or Avro datasets, or want to share only selected sources or tables.

### Review and share

Share only the reviewed `blueprint.toml` or packed bundle by default. A deck
may accompany it only after its content and confidentiality label have been
reviewed and separately approved under your organization's policy.

Keep audits, command records, and unapproved decks local and
access-controlled. They may contain endpoints, authenticated principals, local
paths, timing data, and manifest identifiers. Send them only for a specific
support need through an approved secure channel. Never include password or
token files, anonymization keys, CA private keys, database dumps, or database
logs with a shared Blueprint.

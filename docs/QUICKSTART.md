# Quickstart

This quickstart is written for a sales engineer, DBA, or security reviewer who needs to produce a shareable DBWarp Blueprint file without exposing customer data.

## 1. Choose How to Run the Tool

Use one of these paths:

- Download a release binary and verify its checksum.
- Build from source with `./build.sh`.
- Build from the vendored release bundle for strict offline dependency review.

See [`../BUILD.md`](../BUILD.md) and [`../binaries/README.md`](../binaries/README.md).

Select a presentation language explicitly when required:

```bash
./dbwarp-blueprint --lang fr --help
./dbwarp-blueprint --lang pl --connect postgresql://db.internal/payments --dry-run
```

Supported values are `en`, `de`, `fr`, `es`, `pl`, `ja`, and `zh`. The
presentation language changes help, prompts, diagnostics, progress text, and
deck prose. It never changes option names, accepted values, URI schemes,
selectors, DBP codes, audit keys, or Blueprint TOML. See
[`INTERNATIONALISATION.md`](INTERNATIONALISATION.md).

## 2. Prepare Credentials Safely

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
printf '%s' 'DOMAIN\\migration_user' > /etc/dbwarp/db.user
```

Then use `--user-file /etc/dbwarp/db.user`.

## 3. Dry-Run First

A dry run validates arguments and prints the planned action without connecting:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
  --user-file /etc/dbwarp/db.user \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --dry-run
```

For `--from-toml` deck mode, dry-run is a local preflight and does not read the database.

For multiple customer sources, dry-run the batch manifest instead:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

## 4. Run Catalog-Only Mode

This strict catalog-only mode reads table metadata and statistics, but no row
samples or non-table object catalogs:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

## 5. Choose Non-Table Artifact Detail

The default `--artifact-detail summary` reads non-table catalogs but not object
definitions. It emits bounded counts and external-prerequisite classes. Use
`--artifact-detail none` if policy forbids those catalogs.

For anonymous dependency topology, use `graph`. For bounded language-feature
and complexity bands, use `analyzed`. Both require explicit consent:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

## 6. Run Tier 2 Compression Measurement

Tier 2 reads bounded row samples into memory, computes aggregate compression,
null-density, cardinality/frequency, length, and style measurements, and
discards the sampled values:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

Use Tier 2 when possible. It gives DBWarp better estimates of wire bytes, egress cost, and synthetic text/binary data generation.

## 7. Generate a Deck

During the live run:

```bash
./dbwarp-blueprint \
  --connect postgresql://db.internal/payments \
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

## 8. Review Before Sharing

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

## 9. Handoff to DBWarp

Minimum handoff:

```text
blueprint.toml
```

For a multi-source customer review, create and inspect a packed bundle rather
than handing off the working directory:

```bash
./dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
less customer-blueprint-bundle.packed.toml
```

Bundle metadata retains the source ids, tags, and dataset-group ids chosen in
the batch manifest. Use anonymous values and review them before transfer.

Use `docs/BATCH_AND_BUNDLES.md` when the customer has multiple databases, multiple Parquet or Avro datasets, or wants to approve only selected sources/tables for benchmark generation.

Keep these as access-controlled local evidence by default:

```text
blueprint.audit.txt
blueprint.pptx
command-used.redacted.txt   # optional; created and reviewed by the operator
```

The tool does not create `command-used.redacted.txt`; include it only if the
operator deliberately records and redacts the approved invocation. Audits and
saved commands can contain database endpoints, authenticated
principals, local paths, timing data, and manifest source ids. Send them only
for a specific support need through an approved secure channel. Do not send
password files, CA private keys, customer dumps, or database logs.

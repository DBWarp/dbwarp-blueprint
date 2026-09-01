# Troubleshooting

Common `dbwarp-blueprint` failures and what to do next.

Operator-owned failures now start with a stable `DBPnnnnS` message code, for example `DBP1001E`.
Use the code when searching docs or opening a support ticket. See [Operator Message Codes](MESSAGES.md).

## Requested Language Is Not Used

Use an explicit supported value when diagnosing locale selection:

```bash
dbwarp-blueprint --lang pl --help
```

Supported values are `en`, `de`, `fr`, `es`, `pl`, `ja`, and `zh`. Without
`--lang`, the tool checks `DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES`, and
`LANG` in that order. An unsupported explicit value is rejected with
`DBP1011E`; an incomplete embedded catalog fails startup with `DBP1010E`
rather than falling back to English. On Windows the locale variables are
usually absent, so pass `--lang` or set `DBWARP_BLUEPRINT_LANG`.

## Banner Width or Colours Look Wrong

The banner width comes from `COLUMNS` when it is set, otherwise from the
console on Linux and macOS, otherwise 80 columns. Colour capability comes from
`NO_COLOR`, `TERM`, and `COLORTERM`; when `TERM` is absent, which is normal on
Windows, 16-colour output is used. Override with `--color always` or
`--color never`, or set `COLUMNS` explicitly.

## Password in URI Is Refused

Symptom:

```text
DBP1001E refusing to use URI-embedded password
```

Fix: remove the password from the URI and use one of:

```bash
--password-file /path/to/pass
--password-env DBWARP_BLUEPRINT_PASSWORD
```

The file mode must not allow group/other read on Unix.

## Password File Permission Error

Symptom: the tool rejects `--password-file` or `--tls-key` because permissions are too broad.

Fix:

```bash
chmod 600 /etc/dbwarp/db.pass
chmod 600 /etc/dbwarp/client.key
```

This prevents accidental disclosure through local users on the same host.

## TLS Verification Fails

Use `--tls-mode verify-full` with the correct CA bundle and hostname:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

If the certificate hostname does not match, fix the DNS name or certificate. `--tls-skip-verify` is refused on non-loopback hosts unless `--i-know-what-im-doing` is also supplied; do not use it in production.

## SQL Server TLS Trust Roots

For SQL Server, certificate-verifying modes use the operating-system trust
store when `--tls-ca` is omitted. A supplied `.pem` or `.crt` file must contain
exactly one CA certificate and replaces those roots. The driver checks the
connection hostname in both `verify-ca` and `verify-full`.

## Tier 2 Requires Consent

Symptom:

```text
--measure-compression requires --yes
```

Fix:

```bash
--measure-compression --yes
```

This is intentionally explicit because Tier 2 reads bounded row samples into memory before discarding them.

## Sampling Takes Too Long

Reduce one or both:

```bash
--sample-rows 500
--max-wall-secs 120
```

For the first production review, a smaller Tier 2 sample is better than no compression measurement. If results are biased or incomplete, rerun from a replica with a larger budget.

## DBA Forbids Non-Catalog SELECT 1 Probe

Disable the RTT probe:

```bash
--no-rtt-probe
```

The default RTT probe is five `SELECT 1` queries and does not read row data, but some policies classify any non-catalog query as out of scope.

## Output Contains No Compression Sections

Compression sections appear only when both flags are present:

```bash
--measure-compression --yes
```

Catalog-only Blueprints are valid, but downstream compression estimates will be inferred.

## Some Compression Samples Are Marked Biased

Some engines do not provide uniform table sampling in all cases, and small tables may require a `LIMIT` fallback. The Blueprint file records `sampled_with_bias` and `bias_reason` so the estimator and reviewer can account for it.

Biased samples are still useful; they are just not as strong as uniform samples.

## Deck Generation Fails From TOML

`--from-toml` must be paired with `--deck`:

```bash
./dbwarp-blueprint --from-toml blueprint.toml --deck blueprint.pptx
```

Do not include live database flags with `--from-toml`. The tool rejects mixed live/offline modes to keep the audit boundary simple.

## Blueprint File Looks Too Small

A normal Blueprint file is compact. It contains structural metadata, rounded counts, indexes, FK graph shape, and optional compression summaries. It should not contain row values or identifiers.

If you need a representative benchmark database, pass the approved
`blueprint.toml` to the separately reviewed downstream tooling authorized for
that engagement.

## Need to Prove No Upload Happened

Use the audit log and network tools:

```bash
./dbwarp-blueprint ... --audit-log audit.txt
strace -f -e trace=connect ./dbwarp-blueprint ...
tcpdump host db.internal
```

Expected runtime network behavior depends on the active mode. A live `--connect`
run opens the requested database session; DNS may contact the configured
resolver, and integrated Kerberos/SSPI authentication may contact a KDC or
domain controller. Batch mode opens one database session per database source.
Local TOML, Parquet, Avro, and bundle operations initiate no application
network connection, although network-mounted paths remain subject to the host
storage stack.

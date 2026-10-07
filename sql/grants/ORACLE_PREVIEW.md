# Oracle Basic preview: grants and account lifecycle

Oracle capture in DBWarp Blueprint 1.6 provides the Basic catalogue tier. It
is supported in 1.6 as a preview for Oracle 12c, 19c, 21c and 23ai/26ai, with
the limitations below.
Use it only after reviewing the known limitations below and explicitly
acknowledging the preview in the command line.

The hands-off DBA capture pack is documented in
[`../capture/README.md`](../capture/README.md).

## Grant profiles

Oracle has two catalogue-only profiles:

| Oracle family | Blueprint floor | Optional Basic refinements | Table rows read? | Apply with |
|---|---|---|---|---|
| 12.1 / 12.2, 19c, 21c, 23ai / 26ai | `oracle-*/minimum.sql` | `oracle-*/basic.sql` | No | `sqlplus admin@//HOST:1521/PDB @FILE NO` |

`minimum.sql` grants direct `SELECT` on five SYS-owned dictionary views:
`DBA_OBJECTS`, `DBA_TABLES`, `DBA_TAB_COLUMNS`, `DBA_SEGMENTS`, and
`V_$INSTANCE`. The first four provide table classification, table and column
structure, optimiser row evidence, and segment-byte input. `V_$INSTANCE`
selects version-safe catalogue projections. The collector also receives
`CREATE SESSION` and a privilege-free lifecycle marker role. It receives no
`SELECT` privilege on application tables.

`basic.sql` adds the optional classification, index, relationship, partition,
LOB, statistics, topology, and source-environment catalogues. If those
catalogues are not approved, the minimum profile still produces the Blueprint
floor and records the refinements as incomplete or unknown.

Readable segment bytes are retained when attribution is incomplete. When
storage for a table cannot be attributed, Blueprint may use the labelled
`NUM_ROWS * AVG_ROW_LEN` logical estimate. That weaker result is reported as
partial and incomplete, never as measured segment bytes.

## Account lifecycle

The scripts never create an account or handle its password. An Oracle
administrator must:

1. Create a local collector account in the intended PDB under the site's
   authentication and password policy.
2. Set the collector and marker-role names once in `sql/oracle-accounts.sql`.
3. Connect directly to the PDB, not `CDB$ROOT`.
4. Run the selected `minimum.sql` or `basic.sql` file with `NO`, the normal
   recovery argument.

For example:

```text
sqlplus admin@//HOST:1521/APPPDB @sql/grants/oracle-19c/minimum.sql NO
```

The administrator needs `CREATE ROLE`, `GRANT ANY ROLE`, and `DROP ANY ROLE`
for the recoverable lifecycle marker, plus authority to issue the listed object grants.
These are administrator prerequisites only; they are not granted to the
collector account.

After capture, use the matching `sql/revoke/oracle-*.sql` file. The revoke
script accepts a correctly provisioned subset of the Basic grants, verifies
the local account and marker role, refuses `CDB$ROOT`, and requires `NO` as its
argument.

## Live preview command

The live SQL*Plus preview requires an explicit SQL*Plus executable and an
empty private network-configuration directory:

```text
dbwarp-blueprint \
  --connect oracle://HOST:1521/SERVICE \
  --user dbwarp_blueprint_basic \
  --password-file /private/path/oracle.pass \
  --schema APP_SCHEMA \
  --oracle-sqlplus /absolute/path/sqlplus \
  --oracle-network-config-dir /private/empty-directory \
  --acknowledge-oracle-preview \
  --artifact-detail none \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

For a capture produced entirely by the DBA's SQL*Plus environment, use the
capture pack described in [`../capture/README.md`](../capture/README.md).

## Known limitations

- **Live capture runs on Linux only.** On Windows the binary refuses the live
  Oracle command above with `DBP1426E` before it starts SQL*Plus or opens a
  connection. On Windows, capture with the DBA pack (`capture.ps1`) and
  convert the spool with `--from-oracle-basic`; that path is available on
  both platforms.
- **The Oracle options are not listed in `--help`.** They are the ones shown
  in this document and in the capture pack README.
- **Basic tier only.** Encrypted transport, wallets and external
  authentication are not supported.
- **Oracle 21c and mixed SQL*Plus versions.** The DBA capture scripts can
  fail with ORA-03106 or ORA-03120 when SQL*Plus clients of different major
  versions run them against the same 21c database while the earlier run's
  statements are still cached. Use one SQL*Plus major version per database,
  or have the DBA flush the shared pool before changing client version.

## Information boundary

Oracle cannot grant selected columns of a dictionary view. Direct access to
the named `DBA_*` views can expose dictionary-held values that Blueprint does
not project, including column low/high values, default expressions, and
constraint conditions. `SELECT_CATALOG_ROLE` is broader still. Prefer the
named grants and review this residual visibility explicitly.

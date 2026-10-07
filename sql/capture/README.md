# Oracle Basic DBA capture pack

These SQL*Plus scripts let a DBA create an Oracle Basic capture for DBWarp
Blueprint. Oracle Basic capture is supported in 1.6 as a preview. The DBA runs
the launcher on the
database host; it selects the matching version-family script automatically.
The `dbwarp-blueprint` binary does not run there
and does not need database-network access. The resulting spool is transferred
to a separate computer with no database access and converted with
`--from-oracle-basic`.

Use `capture.sh` with Bash 3 or later on Unix, or `capture.ps1` on Windows.
The Unix path uses only baseline host utilities (`awk`, `cksum`, `od`, `sed`,
`sort`, `tail`, and `tr`) and does not require a separate SHA-256 program. The launcher detects
Oracle 12.1, 12.2, 19c, 21c, or the major-23 catalogue line used by 26ai and
selects the matching generated script. Connect as the dedicated collector
account provisioned with `minimum.sql` or `basic.sql`; do not connect as `SYS`,
`SYSTEM`, or a broadly privileged DBA account. The connection argument is
`username@service` without a password, so SQL*Plus prompts for the password
instead of exposing it in the process arguments.

`capture.sh` is a Bash launcher, not a `/bin/sh` script. AIX, HP-UX, and other
Unix hosts without Bash cannot run it directly. Run the pack from a supported
Bash-equipped host that has SQL*Plus and database network access, or use the
Windows PowerShell launcher; do not invoke `capture.sh` through `/bin/sh`.

The 26ai script intentionally accepts Oracle's major-23 catalogue contract.
`V_$INSTANCE.VERSION` reports the base `23.0.0.0.0` value used for minimum-tier
automatic selection, while a 26ai database reports its 23.26 release
update through `PRODUCT_COMPONENT_VERSION.VERSION_FULL` and the full banner.

Pass ordinary Oracle owner names. The launcher UTF-8 encodes them, sorts them
by encoded bytes, checks for duplicates, and passes only the safe hex form to
SQL*Plus. For example, capture `APP` and `REPORTING` on Unix with:

```sh
mkdir oracle-blueprint-capture
cd oracle-blueprint-capture
/path/to/capture-pack/capture.sh BPCOLLECTOR@service APP REPORTING
```

On Windows PowerShell:

```powershell
New-Item -ItemType Directory oracle-blueprint-capture
Set-Location oracle-blueprint-capture
& C:\path\to\capture-pack\capture.ps1 BPCOLLECTOR@service APP REPORTING
```

For a long owner list, put one ordinary owner name on each line. Blank lines
and duplicates are refused. On Unix use `--owners-file owners.txt`; on Windows
use `-OwnerFile owners.txt`. The private bind is a chunked CLOB, so the list is
not constrained by SQL*Plus's 32,767-byte `VARCHAR2` limit. The capture contract
still limits one run to 16,384 owners. The owner file contains native schema
names; keep it private and remove it under the same retention policy as the
spool.

The fixed output name is `dbwarp-blueprint-oracle-basic.spool`. It contains
real schema, table, column, and other catalogue names. Keep it inside your
organisation, transfer it only through an approved private channel, and
delete it under your retention policy after conversion.

Convert it on a computer with no database connection or Oracle
client required:

```text
dbwarp-blueprint \
  --from-oracle-basic dbwarp-blueprint-oracle-basic.spool \
  --acknowledge-oracle-preview \
  --artifact-detail summary \
  --anonymization-key-file /private/path/blueprint.key \
  --out blueprint.toml \
  --audit-log blueprint.audit.txt
```

The Unix launcher sets `umask 077` and keeps its bind file, raw SQL*Plus spool,
sanitized transfer spool, and partial output private. The PowerShell launcher
does the same with an owner-only ACL. Both launchers refuse to overwrite an
existing capture file and check the selected generated script for accidental
damage: PowerShell uses its built-in SHA-256 implementation, while Unix uses
POSIX `cksum`. Offline ingestion then verifies the canonical SHA-256 identity
recorded in the spool. The launchers remove the private bind and raw files after
sanitization and leave only the final spool. If a transfer tool broadens the
permissions, first confirm that the file belongs to the converting account,
then repair it before conversion:

```sh
test -O dbwarp-blueprint-oracle-basic.spool
chmod 600 dbwarp-blueprint-oracle-basic.spool
```

If the ownership check fails, copy the file through the approved private
channel as the converting account; do not weaken the converter's permission
check. The converter refuses an unsafe Unix mode rather than silently reading
a spool exposed to other users.

Launcher refusals use `DBP1426E` and give the next local check. A failure
before the frame header usually means the login failed, the selected PDB or
server family is wrong, an owner name does not exist with that exact case, or
the collector grants are incomplete. The launchers deliberately do not copy
Oracle message text into the transferable spool.

For controlled automation, `run.sql` is the auto-detecting SQL*Plus entry
point. The launchers write its private, chunked CLOB bind file; no native owner
name or long owner list is substituted into SQL text or passed as a SQL*Plus
argument. Ordinary DBA use should prefer the launchers. The generated family
scripts are
`oracle-12c/basic-12.1.sql`, `oracle-12c/basic-12.2.sql`,
`oracle-19c/basic.sql`, `oracle-21c/basic.sql`, and
`oracle-26ai/basic.sql`.

The scripts are generated from the collector's Oracle catalogue queries and are
bound to the released binary by their recorded digest; do not edit them by
hand. Rows are spooled as they are fetched rather than accumulated in
`DBMS_OUTPUT`, and the scripts do not require `DBMS_SQL`. Query failures are
sanitized locally to a closed class and number; Oracle message text never
enters the transferable spool. A cancellation or lost session retains the
completed prefix as a classified partial capture instead of turning it into an
empty file. A complete spool is still accepted when a slow catalogue takes
longer than the live SQL*Plus capture deadline; offline conversion performs
no further database work and records the observed duration as provenance.

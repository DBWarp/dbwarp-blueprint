# Operator Message Codes

`dbwarp-blueprint` uses stable operator message identifiers for DBWarp-owned validation and workflow failures.
The format is inspired by IBM-style operator messages: a subsystem prefix, a numeric identifier, and a severity suffix.
IBM CICS documentation describes a program identifier plus a four-digit message number and severity letter; IBM MQ similarly uses component/prefix fields, a numeric identifier, and a final message type code. Microsoft error-message guidance reinforces the practical rule that an error should describe the problem and provide an action the user can take.

References:

- IBM CICS message format: https://www.ibm.com/docs/en/cics-pa/5.3.0?topic=messages-message-format
- IBM CICS message information layout: https://www.ibm.com/docs/en/cics-ts/6.x?topic=messages-format-cics-message-information
- IBM MQ for z/OS message format: https://www.ibm.com/docs/SSFKSJ_9.2.0/com.ibm.mq.ref.doc/q050270_.htm
- Microsoft error-message guidance: https://learn.microsoft.com/en-us/windows/win32/uxguide/mess-error

## Format

```text
DBPnnnnS message text. Next: corrective action.
```

Fields:

- `DBP` means DBWarp Blueprint.
- `nnnn` is a stable four-digit message number.
- `S` is severity: `E` error, `W` warning, `I` information.

The code is stable and language-neutral. Its summary, cause, and corrective
action are localized when `--lang` or the process locale selects a supported
language. Dynamic operating-system, database-driver, path, and causal-chain
details remain verbatim so support engineers can search the original failure.
Message text must not include secrets or unredacted connection URIs.

## Ranges

| Range | Area |
|---|---|
| `DBP0001E` | Genuinely unclassified wrapped failure with causal chain |
| `DBP10xxE` | Command, connection input, and collection-policy validation |
| `DBP11xxE` | Batch manifest and source input validation |
| `DBP12xxE` | Bundle selectors and Blueprint URI selectors |
| `DBP13xxE` | Offline TOML/deck/schema validation |
| `DBP14xxE/W` | Live database capture failures and non-fatal sampling degradation |
| `DBP15xxE/W` | Structured-file, Blueprint, deck, and audit output |
| `DBP16xxE/W` | Credential, authentication, TLS, and sensitive-file policy |
| `DBP17xxE` | Operator consent |
| `DBP18xxE` | Process runtime initialization |

## Current Codes

| Code | Meaning |
|---|---|
| `DBP0001E` | Unclassified failure; causal chain follows. |
| `DBP1000E` | `--connect` missing outside offline modes. |
| `DBP1001E` | URI-embedded password refused. |
| `DBP1002E` | Unsupported `--connect` URI scheme. |
| `DBP1003E` | Unsupported TLS server-name override. |
| `DBP1004E` | Azure token flag used with a non-SQL Server engine. |
| `DBP1005E` | Authentication mode is unavailable for the selected engine. |
| `DBP1006E` | Structured-file compression sampling requested without explicit `--yes`. |
| `DBP1007E` | Explicit length-fidelity mode requested for an engine that does not yet expose that contract. |
| `DBP1008E` | Legacy exact-length alias conflicts with strict length fidelity. |
| `DBP1009E` | Exact sampled-length fidelity requested without explicit `--yes`. |
| `DBP1010E` | Embedded localization catalog is incomplete or inconsistent. |
| `DBP1011E` | Command-line arguments are invalid. |
| `DBP1012E` | A supported database connection URI is malformed. |
| `DBP1013E` | `--source-kind` is empty or unsupported. |
| `DBP1014E` | Anonymous artifact graph or definition analysis requested without explicit consent. |
| `DBP1015E` | TLS client-certificate options used with SQL Server, whose driver does not implement them. |
| `DBP1101E` | Batch manifest cannot be read. |
| `DBP1102E` | Batch manifest cannot be parsed. |
| `DBP1103E` | Batch manifest has no `[[source]]` entries. |
| `DBP1104E` | Batch mode needs explicit `--yes`. |
| `DBP1105E` | One source inside a batch failed. |
| `DBP1106E` | Unsupported batch source kind. |
| `DBP1107E` | File source resolved no input files. |
| `DBP1108E` | Unsupported file dataset mode. |
| `DBP1109E` | Batch source identifier has no usable ASCII letter or digit. |
| `DBP1110E` | Database source has the wrong number of connection sources. |
| `DBP1111E` | `connect_env` variable is missing or unreadable. |
| `DBP1112E` | `connect_file` is missing or unreadable. |
| `DBP1113E` | Batch output, audit, report, or directory could not be completed. |
| `DBP1114E` | Structured-file dataset members are incompatible. |
| `DBP1115E` | Every batch source failed; only diagnostic output was published. |
| `DBP1116E` | A partial batch bundle was published. |
| `DBP1200E` | Invalid selector or `blueprint://` syntax. |
| `DBP1201E` | Bundle selector matched no sources. |
| `DBP1202E` | Bundle selector matched multiple sources. |
| `DBP1203E` | Bundle selector matched no extractable Blueprint/table. |
| `DBP1204E` | Bundle input could not be read. |
| `DBP1205E` | Bundle or referenced Blueprint content is invalid. |
| `DBP1206E` | Bundle output could not be written. |
| `DBP1301E` | `--from-toml` missing `--deck`. |
| `DBP1302E` | Unsupported Blueprint TOML schema version. |
| `DBP1401E` | PostgreSQL capture boundary failed. |
| `DBP1402E` | MySQL capture boundary failed. |
| `DBP1403E` | SQL Server capture boundary failed. |
| `DBP1404W` | Loopback PostgreSQL TLS `prefer` mode fell back to plaintext. |
| `DBP1405W` | Optional database RTT probe was unavailable. |
| `DBP1406W` | Tier 2 sampling time budget was exhausted. |
| `DBP1407W` | A compression sample was unavailable. |
| `DBP1408W` | A text-column style sample was unavailable. |
| `DBP1409W` | PostgreSQL's asynchronous connection task reported an error. |
| `DBP1410W` | An optional artifact catalog was unavailable, so completeness is explicitly reduced. |
| `DBP1411W` | Topology evidence is unavailable; deployment and local role remain unknown. |
| `DBP1412W` | A distributed or sharded layout was detected but complete aggregate sizing was unavailable. |
| `DBP1413W` | Dataset table, row, or byte coverage is incomplete or unknown. |
| `DBP1414W` | Bundle source relationship is unknown, so cross-source arithmetic is unsafe. |
| `DBP1415W` | Declared replicas disagree; one deterministic representative was retained without averaging. |
| `DBP1416W` | A shard group is incomplete and contributes no aggregate totals. |
| `DBP1417W` | Bundle aggregate totals were suppressed. |
| `DBP1418W` | A source included in bundle arithmetic has incomplete or unknown dataset coverage. |
| `DBP1419E` | Live capture exceeded `--max-wall-secs`; the client dropped the connection and reports the engine-specific server limit. |
| `DBP1420E` | At least one requested `--schema` was not visible, so no ambiguously scoped Blueprint was written. |
| `DBP1421W` | SQL Server session principal evidence was unavailable; capture continued without an identity claim. |
| `DBP1501E` | Structured-file capture boundary failed. |
| `DBP1502E` | Blueprint or bundle output failed. |
| `DBP1503E` | PowerPoint deck generation failed. |
| `DBP1504W` | Audit log could not be written. |
| `DBP1601E` | Credential acquisition failed. |
| `DBP1602E` | TLS configuration failed. |
| `DBP1603E` | Database username acquisition failed. |
| `DBP1604E` | Database authentication configuration is invalid. |
| `DBP1605W` | Sensitive-file permission enforcement is unavailable on this platform. |
| `DBP1606E` | The authenticated SQL Server principal assertion failed before catalog capture. |
| `DBP1607E` | The anonymization HMAC key could not be initialized safely. |
| `DBP1701E` | Operation was cancelled before explicit consent. |
| `DBP1702E` | Consent response could not be read from standard input. |
| `DBP1801E` | The asynchronous runtime could not be initialized. |

Every advertised language must contain every current DBP summary, cause, and
action. The binary validates this at startup and fails with `DBP1010E` rather
than silently falling back to English.

Predictable decision-boundary failures are exercised by an adversarial CLI
matrix. A known condition must emit its specific code as the first operator
code and must not fall back to `DBP0001E`. The renderer also scans the complete
error chain so an uncoded implementation context cannot hide a coded inner
cause.

Non-fatal database sampling warnings are printed with their stable warning
code and recorded in the run audit. This distinguishes a complete Tier 2
capture from a successful but partially sampled capture without turning an
optional probe failure into a total collection failure.

## Support Checklist

When a customer reports a failure, ask for:

- the complete terminal output, including the `DBP` code;
- the audit log if `--audit-log` was used;
- the redacted command line;
- for bundle errors, the output of `dbwarp-blueprint --bundle-list ...`.

Do not ask for password files, token files, private keys, or raw database row samples.

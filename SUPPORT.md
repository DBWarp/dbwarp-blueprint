# Support and problem reports

Use the [issue tracker](https://github.com/DBWarp/dbwarp-blueprint/issues) for
non-sensitive installation questions, reproducible defects and feature
requests. This channel does not promise a response time or a support service
level.

Report suspected vulnerabilities privately using the route in
[SECURITY.md](SECURITY.md), not a public issue.

## Information that helps

- Exact release tag, binary checksum, operating system and architecture.
- Database engine and version, or structured-file format, and whether the
  source is self-managed or managed.
- Command options with credentials, endpoints, paths and identifying selectors
  removed or replaced by clearly marked examples.
- The `DBP` diagnostic code, expected behavior and actual behavior.
- A small synthetic reproduction when possible.

Do not upload production data, credentials or an unreviewed audit, bundle,
Blueprint or deck. Audits can contain identities and endpoints; anonymous
Blueprints can still reveal distinctive workload structure. Start with the
minimum safe description, and review each attachment before sharing it.

## Supported configurations

[STATUS.md](STATUS.md) describes capabilities and the qualified engine matrix.
[BUILD.md](BUILD.md) describes platform and authentication-specific build
requirements. Rust is pinned to the exact version in `rust-toolchain.toml`;
the package's `rust-version` is not a promise that every newer toolchain is
qualified.

Managed-service permission guidance is not a claim that every service or
configuration has been tested. Use the matching
[permission requirements](sql/grants/DATABASE_PERMISSIONS.md) and qualify the
exact configuration before production use.

For changes between collector versions, see [CHANGELOG.md](CHANGELOG.md).

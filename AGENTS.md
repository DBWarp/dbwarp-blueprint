# Guidance for AI coding agents

This repository is the source for DBWarp Blueprint and its `dbwarp-blueprint`
binary: a local command-line collector that reads database catalog metadata
and writes an anonymized, reviewable `blueprint.toml`. This file tells an
agent how to help someone run the tool or change the code without weakening
its safety properties.

## Where to look first

| Task | Read |
|---|---|
| What the tool does and its options | [README.md](README.md), [docs/QUICKSTART.md](docs/QUICKSTART.md) |
| Building from source | [BUILD.md](BUILD.md) |
| Database accounts and grants | [sql/grants/README.md](sql/grants/README.md) |
| What a run reads, writes and contacts | [AUDIT.md](AUDIT.md), [SECURITY.md](SECURITY.md) |
| Output format | [FORMAT.md](FORMAT.md) |
| A `DBPnnnnS` message | [docs/MESSAGES.md](docs/MESSAGES.md), [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md) |
| Proposing a change | [CONTRIBUTING.md](CONTRIBUTING.md) |

English documentation is authoritative. The translated documents under
`docs/<language>/` are machine translations and may contain errors; see
[MACHINE_TRANSLATIONS.md](MACHINE_TRANSLATIONS.md).

## Helping someone run the tool

- Build with `./build.sh`. It uses the pinned toolchain and locked
  dependencies and refuses to download anything unless `ALLOW_NETWORK=1` is
  set. Ask before setting it. The binary is `target/release/dbwarp-blueprint`.
- Start with `--dry-run`. It validates the arguments and prints the plan
  without connecting.
- Use a dedicated least-privilege account created by the DBA from the matching
  script under `sql/grants/`. Do not suggest an owner, administrator,
  superuser, `root`, `sa` or `db_owner` account, and do not run grant or revoke
  scripts yourself.
- Never put a password in the connection URI; the tool refuses it. Use
  `--password-file`, `--password-env` or the prompt. Do not read, print or
  copy password, token or key files.
- Row sampling (`--measure-compression`) and the `graph` and `analyzed`
  artifact levels need explicit consent. Add `--yes` only when the person has
  approved that read; do not add it to make a command pass.
- Keep TLS verification on. Do not suggest `--tls-skip-verify` outside a
  loopback test.
- The output stays local. Review `blueprint.toml` and the audit log with the
  person before anything is shared, and never upload either file on your own.

## Changing the code

- Keep `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `build.sh` and
  `BUILD.md` consistent. Do not change a dependency version without also
  updating the lock file and `THIRD_PARTY_NOTICES.md`.
- Option names, accepted values, DBP codes, audit keys and TOML field names
  are stable interfaces and stay in English. Identifiers from earlier schema
  versions are accepted on input only and are never written.
- A change to help text, a prompt or a diagnostic must update `src/i18n.rs`,
  every catalog under `locales/` and [docs/MESSAGES.md](docs/MESSAGES.md) in
  the same change. The binary refuses to start if a catalog is incomplete.
- Preserve the safety properties: explicit consent, bounded reads, least
  privilege, no telemetry, and honest reporting of missing or degraded
  observations. Do not turn an unknown into a zero or a complete result.
- Never add credentials, real hostnames, customer or database object names, or
  captured row values to code, tests, fixtures or documentation. Use synthetic
  examples.
- Do not hand-edit the translated documents under `docs/<language>/`.

Run these before proposing a change:

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

Say which databases, versions and platforms you actually tested. Passing local
tests does not show that a change works on every supported configuration.

## Not for an agent to do

- Publish a release, move a tag or change repository settings.
- Report a suspected vulnerability in a public issue; follow
  [SECURITY.md](SECURITY.md).
- Attach a real Blueprint, audit log or database output to an issue without
  the owner's review; follow [SUPPORT.md](SUPPORT.md).

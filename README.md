<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/dbwarp-logo-dark.png">
    <img src=".github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

## Publishing shortly

Source and binaries are being published here within the next few days.

**Watch this repository** to be notified the moment the first release lands. Use the Watch button
above, choose Custom, then tick Releases. Or simply check back shortly.

## What it is

DBWarp Blueprint is a trust-first database blueprint collector. You run it inside your own
environment against PostgreSQL, MySQL or SQL Server. It reads catalogue metadata only, and writes
an anonymised structural blueprint of your database: table sizes, row counts, type families, index
and foreign-key shape. No schema names, no column names, no row data.

The output is a plain-text file. You can read every line of it before deciding whether to share it.

DBWarp Blueprint is free and open source, and it runs entirely inside your environment. It exists
so you can give us facts about your database without giving us your database.

## Why you would run it

Share your Blueprint output with us and we can tell you how much faster DBWarp would move your
data, and what that changes for your migration, CI/CD test-data and analytics timelines.

The distance matters most. The further your data has to travel, the bigger the improvement DBWarp
can show you.

## What lands at release

- Full source, dual licensed under Apache-2.0 or MIT, with a vendored dependency bundle for strict
  offline audit
- Binaries for Linux, macOS and Windows, with SHA-256 checksums for every artefact
- Build-from-source instructions. Building it yourself is the trust-first path, and the binaries
  exist for convenience on quick trials

## In the meantime

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zürich, Switzerland

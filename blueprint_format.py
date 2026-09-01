#!/usr/bin/env python3
"""
blueprint_format.py — normalize JSON output from sql/blueprint.{pg,mysql,sqlserver}.sql
into the canonical dbwarp-blueprint TOML format.

The Rust contract in crates/dbwarp-blueprint-core is authoritative. This
stdlib-only fallback is held to byte-level fixtures exercised by the Rust test
suite; update the core contract and those cross-language snapshots together.

Stdlib-only; no pip install required. Tested with Python 3.8+.

Usage:
    python3 blueprint_format.py [--source-kind=production]
        [--anonymization-key-file=PATH] blueprint.json > blueprint.toml

Trust contract:
    - Reads the JSON file you pass it (or stdin) and only the optional
      customer-selected anonymization key file.
    - Writes only to stdout.
    - No network, no environment variable reads, no /tmp scratch.
    - Identifier ordering uses domain-separated HMAC-SHA256. By default a
      fresh key comes from operating-system randomness. Supply the same
      protected key file and pin generated_at only when stable labels and
      byte-identical output are explicitly required.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import hmac
import json
import os
import re
import secrets
import stat
import sys
from typing import Any, Dict, List, Optional, Tuple


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "input",
        nargs="?",
        default="-",
        help="path to JSON output from blueprint.{engine}.sql (default: stdin)",
    )
    parser.add_argument("--source-kind", default="production")
    parser.add_argument(
        "--generated-at",
        default=None,
        help="pin generated_at field to a specific UTC timestamp "
        "(ISO 8601, no fractional seconds). Default: current UTC.",
    )
    parser.add_argument(
        "--anonymization-key-file",
        default=None,
        help="path to a protected file containing exactly 32 raw bytes or "
        "64 hexadecimal characters; omit for a fresh process-local key",
    )
    args = parser.parse_args()

    if args.input == "-":
        raw = sys.stdin.read()
    else:
        with open(args.input, "r", encoding="utf-8") as f:
            raw = f.read()

    # psql with -A -t emits a single line of JSON; trim it.
    raw = raw.strip()
    if not raw:
        sys.stderr.write("blueprint_format: input is empty\n")
        return 2
    data = json.loads(raw)

    if data.get("schema_version") not in (1, 2):
        sys.stderr.write(
            f"blueprint_format: unsupported schema_version {data.get('schema_version')}; "
            f"expected 1 or 2\n"
        )
        return 2

    source_kind = _validate_source_kind(args.source_kind)
    generated_at = args.generated_at or _utc_iso_now()
    try:
        anonymization_key, key_source = _load_anonymization_key(
            args.anonymization_key_file
        )
        out = build_blueprint(
            data,
            source_kind=source_kind,
            generated_at=generated_at,
            anonymization_key=anonymization_key,
            anonymization_key_source=key_source,
        )
    except (OSError, ValueError) as error:
        sys.stderr.write(f"blueprint_format: {error}\n")
        return 2
    sys.stdout.write(out)
    return 0


def _utc_iso_now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


# ---------------------------------------------------------------------------
# Anonymization
# ---------------------------------------------------------------------------

_ANONYMIZATION_PREFIX = b"dbwarp-blueprint-anonymization-v1"
_ANONYMIZATION_KEY_BYTES = 32


def _load_anonymization_key(path: Optional[str]) -> Tuple[bytes, str]:
    if path is None:
        return secrets.token_bytes(_ANONYMIZATION_KEY_BYTES), "ephemeral-random"

    if os.name == "posix":
        mode = stat.S_IMODE(os.stat(path).st_mode)
        if mode & 0o044:
            raise ValueError(
                f"anonymization key file {path!r} has mode {mode:#o}; refusing "
                "to read it because group and other read bits must be clear "
                "(recommended: 0600)"
            )

    with open(path, "rb") as key_file:
        raw = key_file.read(4097)
    if len(raw) > 4096:
        raise ValueError("anonymization key file is unreasonably large")
    if len(raw) == _ANONYMIZATION_KEY_BYTES:
        return raw, "customer-key-file"

    try:
        encoded = raw.decode("utf-8").strip()
    except UnicodeDecodeError as error:
        raise ValueError(
            "anonymization key file must contain exactly 32 raw bytes or "
            "64 hexadecimal characters"
        ) from error
    if len(encoded) != 64 or not re.fullmatch(r"[0-9A-Fa-f]{64}", encoded):
        raise ValueError(
            "anonymization key file must contain exactly 32 raw bytes or "
            "64 hexadecimal characters"
        )
    return bytes.fromhex(encoded), "customer-key-file"


def _keyed_hash8(key: bytes, domain: bytes, *components: str) -> bytes:
    mac = hmac.new(key, digestmod=hashlib.sha256)
    mac.update(_ANONYMIZATION_PREFIX)
    for component in (domain, *(value.encode("utf-8") for value in components)):
        mac.update(len(component).to_bytes(8, "big"))
        mac.update(component)
    return mac.digest()[:8]


def _table_key(key: bytes, schema: str, name: str) -> bytes:
    return _keyed_hash8(key, b"table", schema, name)


def _schema_key(key: bytes, schema: str) -> bytes:
    return _keyed_hash8(key, b"schema", schema)


def _index_key(key: bytes, index: str) -> bytes:
    return _keyed_hash8(key, b"index", index)


def _schema_letter(idx: int) -> str:
    """1 → A, 26 → Z, 27 → AA, 52 → AZ, ..."""
    if idx <= 0:
        return "?"
    out = ""
    n = idx
    while n > 0:
        n, r = divmod(n - 1, 26)
        out = chr(ord("A") + r) + out
    return out


# ---------------------------------------------------------------------------
# Privacy rounding (matches the collector helpers around the canonical core)
# ---------------------------------------------------------------------------

def _round_to(n: int, bucket: int) -> int:
    if bucket <= 0:
        return n
    return ((n + bucket // 2) // bucket) * bucket


def _round_rows(n: int) -> int:
    if n <= 10_000:
        bucket = 100
    elif n <= 1_000_000:
        bucket = 1_000
    else:
        bucket = 10_000
    return _round_to(n, bucket)


def _round_bytes(n: int) -> int:
    if n < 1_048_576:
        bucket = 1024
    elif n < 1_073_741_824:
        bucket = 1_048_576
    else:
        bucket = 100 * 1_048_576
    return _round_to(n, bucket)


def _round_len_avg(n: int) -> int:
    return _round_to(n, 10)


# ---------------------------------------------------------------------------
# Type normalization
# ---------------------------------------------------------------------------

_PG_BUILTINS = {
    "boolean": "boolean",
    "bool": "boolean",
    "smallint": "smallint",
    "int2": "smallint",
    "integer": "integer",
    "int4": "integer",
    "bigint": "bigint",
    "int8": "bigint",
    "real": "real",
    "float4": "real",
    "double precision": "double precision",
    "float8": "double precision",
    "date": "date",
    "time": "time",
    "time without time zone": "time",
    "time with time zone": "time",
    "timetz": "time",
    "timestamp": "timestamp",
    "timestamp without time zone": "timestamp",
    "timestamp with time zone": "timestamp",
    "timestamptz": "timestamp",
    "uuid": "uuid",
    "json": "json",
    "jsonb": "json",
    "text": "text",
    "character varying": "text",
    "varchar": "text",
    "character": "text",
    "char": "text",
    "bytea": "binary",
    "inet": "network",
    "cidr": "network",
    "macaddr": "network",
    "macaddr8": "network",
    "point": "geometry",
    "line": "geometry",
    "lseg": "geometry",
    "box": "geometry",
    "path": "geometry",
    "polygon": "geometry",
    "circle": "geometry",
}


def _validate_source_kind(s: str) -> str:
    allowed = {"production", "staging", "scrubbed-replica", "synthetic"}
    if s not in allowed:
        sys.stderr.write(
            "blueprint_format: unsupported source_kind "
            f"{s!r}; expected production, staging, scrubbed-replica, or synthetic\n"
        )
        raise SystemExit(2)
    return s


def _type_head(raw: str) -> str:
    return raw.strip().lower().split("(", 1)[0].strip()


def _normalize_pg_type(raw: str) -> str:
    t = raw.strip().lower()
    if t.endswith("[]"):
        return f"array<{_normalize_pg_type(t[:-2])}>"
    head = _type_head(t)
    if head in {"numeric", "decimal"}:
        m = re.fullmatch(r"(?:numeric|decimal)\(([\d,\s]+)\)", t)
        if m:
            return "numeric(" + m.group(1).replace(" ", "") + ")"
        return "numeric"
    return _PG_BUILTINS.get(head, "user-defined")


def _normalize_mysql_type(raw: str) -> str:
    head = _type_head(raw)
    if head in {"tinyint", "smallint", "mediumint", "int", "integer", "bigint"}:
        return "integer"
    if head in {"decimal", "numeric"}:
        return "numeric"
    if head in {"float", "double", "real"}:
        return "float"
    if head in {"bit", "bool", "boolean"}:
        return "boolean"
    if head in {"date"}:
        return "date"
    if head in {"time"}:
        return "time"
    if head in {"datetime", "timestamp"}:
        return "timestamp"
    if head == "json":
        return "json"
    if head in {"char", "varchar", "tinytext", "text", "mediumtext", "longtext", "enum", "set"}:
        return "text"
    if head in {
        "binary",
        "varbinary",
        "tinyblob",
        "blob",
        "mediumblob",
        "longblob",
        "geometry",
        "point",
        "linestring",
        "polygon",
        "multipoint",
        "multilinestring",
        "multipolygon",
        "geometrycollection",
    }:
        return "binary"
    return "user-defined"


def _normalize_mssql_type(raw: str) -> str:
    head = _type_head(raw)
    if head in {"bigint", "int", "smallint", "tinyint"}:
        return "integer"
    if head in {"float", "real"}:
        return "float"
    if head == "bit":
        return "boolean"
    if head in {"varchar", "char", "nvarchar", "nchar", "text", "ntext", "xml"}:
        return "text"
    if head in {"varbinary", "binary", "image", "geography", "geometry", "hierarchyid", "rowversion", "timestamp"}:
        return "binary"
    if head == "uniqueidentifier":
        return "uuid"
    if head == "date":
        return "date"
    if head in {"datetime", "datetime2", "smalldatetime", "datetimeoffset"}:
        return "timestamp"
    if head == "time":
        return "time"
    if head in {"decimal", "numeric"}:
        m = re.fullmatch(r"(?:decimal|numeric)\(([\d,\s]+)\)", raw.strip().lower())
        if m:
            return head + "(" + m.group(1).replace(" ", "") + ")"
        return "numeric"
    if head in {"money", "smallmoney"}:
        return "numeric"
    return "user-defined"


def _normalize_type(engine: str, raw: str) -> str:
    e = engine.strip().lower()
    if e == "postgresql":
        return _normalize_pg_type(raw)
    if e == "mysql":
        return _normalize_mysql_type(raw)
    if e == "sqlserver":
        return _normalize_mssql_type(raw)
    return "user-defined"


def _normalize_index_method(engine: str, raw: str) -> str:
    method = raw.strip().lower()
    if engine == "postgresql" and method in {"btree", "hash", "gin", "gist", "spgist", "brin"}:
        return method
    if engine == "mysql" and method in {"btree", "hash", "fulltext", "spatial", "rtree"}:
        return method
    if engine == "sqlserver" and method in {
        "heap",
        "clustered",
        "nonclustered",
        "xml",
        "spatial",
        "clustered columnstore",
        "nonclustered columnstore",
        "hash",
    }:
        return method
    return "other"


def _normalize_engine_version(engine: str, raw: str) -> str:
    if engine in {"postgresql", "mysql", "sqlserver"}:
        match = re.search(r"[0-9]+(?:[.][0-9]+)+", raw)
        return match.group(0) if match else "unknown"
    return "unknown"


# ---------------------------------------------------------------------------
# Blueprint builder
# ---------------------------------------------------------------------------

def build_blueprint(
    data: Dict[str, Any],
    *,
    source_kind: str,
    generated_at: str,
    anonymization_key: bytes,
    anonymization_key_source: str,
) -> str:
    engine = str(data.get("engine", "postgresql"))
    engine_version = _normalize_engine_version(
        engine, str(data.get("engine_version", ""))
    )
    tables_in: List[Dict[str, Any]] = data.get("tables") or []

    # Secret-keyed ordering prevents an offline reader from testing candidate
    # source names. It is stable only when the customer deliberately reuses a
    # protected key file.
    sorted_tables = sorted(
        tables_in,
        key=lambda t: _table_key(
            anonymization_key, str(t["schema_name"]), str(t["table_name"])
        ),
    )
    # OID type varies by engine: PG uses int, MySQL uses string "schema.table".
    # Normalize to a string key for the lookup table.
    oid_to_id: Dict[Any, str] = {}
    for i, t in enumerate(sorted_tables, start=1):
        oid_to_id[t["oid"]] = f"table-{i:03d}"

    # Name-based engines describe FK columns with source names. Resolve both
    # sides before anonymization so emitted edges contain only stable ordinals.
    column_ordinals_by_oid: Dict[Any, Dict[str, int]] = {}
    for t in sorted_tables:
        column_ordinals_by_oid[t["oid"]] = {
            str(column["name"]).lower(): int(column["attnum"])
            for column in t.get("columns") or []
            if column.get("name")
        }

    # Schema ordinals.
    schema_seen = sorted(
        {str(t["schema_name"]) for t in sorted_tables},
        key=lambda schema: _schema_key(anonymization_key, schema),
    )
    schema_to_id: Dict[str, str] = {
        name: f"schema-{_schema_letter(i + 1)}" for i, name in enumerate(schema_seen)
    }

    # Build per-table Blueprint records.
    totals_table_count = 0
    totals_row_count = 0
    totals_table_bytes = 0
    totals_index_bytes = 0
    table_blueprints: List[Tuple[str, Dict[str, Any]]] = []
    fk_edges_by_from: Dict[str, List[Dict[str, Any]]] = {}

    for t in sorted_tables:
        tid = oid_to_id[t["oid"]]
        rows_raw = max(int(float(t.get("reltuples", 0))), 0)
        table_bytes_raw = max(int(t.get("table_bytes", 0)), 0)
        index_bytes_raw = max(int(t.get("index_bytes", 0)), 0)
        rows = _round_rows(rows_raw)
        table_bytes = _round_bytes(table_bytes_raw)
        index_bytes = _round_bytes(index_bytes_raw)
        totals_table_count += 1
        totals_row_count += rows
        totals_table_bytes += table_bytes
        totals_index_bytes += index_bytes

        cols_in = t.get("columns") or []
        cols_in_sorted = sorted(cols_in, key=lambda c: int(c["attnum"]))
        cols_out: List[Tuple[str, Dict[str, Any]]] = []
        # Build a lower-case-name → ordinal map for index/FK column matching
        # (used by MySQL where indexes reference COLUMN_NAME, not ordinals).
        col_name_to_ord = column_ordinals_by_oid[t["oid"]]
        for c in cols_in_sorted:
            cid = f"col-{int(c['attnum'])}"
            cols_out.append(
                (
                    cid,
                    {
                        "ordinal": int(c["attnum"]),
                        "type": _normalize_type(engine, str(c.get("type", ""))),
                        "nullable": not bool(c.get("not_null", False)),
                        "len_avg": _round_len_avg(max(int(c.get("avg_width", 0)), 0)),
                        "len_p95": 0,
                        "style": "",
                    },
                )
            )

        idxs_in = t.get("indexes") or []
        # PG path: each index entry has col_ords array.
        # MySQL path: each entry is a (name, seq, col_name) row, multiple per index.
        idxs_out: List[Tuple[str, Dict[str, Any]]] = []
        if any("col_name" in i for i in idxs_in):
            # MySQL: group by (name) and sort col_names by seq.
            grouped: Dict[str, List[Dict[str, Any]]] = {}
            for i in idxs_in:
                grouped.setdefault(str(i["name"]), []).append(i)
            for n, name in enumerate(
                sorted(grouped.keys(), key=lambda index: _index_key(anonymization_key, index)),
                start=1,
            ):
                parts = sorted(grouped[name], key=lambda x: int(x.get("seq", 0)))
                col_ords = []
                for p in parts:
                    cn = p.get("col_name")
                    if cn and str(cn).lower() in col_name_to_ord:
                        col_ords.append(col_name_to_ord[str(cn).lower()])
                idxs_out.append(
                    (
                        f"idx-{n}",
                        {
                            "type": _normalize_index_method(engine, str(parts[0].get("method", "btree"))),
                            "unique": bool(parts[0].get("unique", False)),
                            "cols": col_ords,
                        },
                    )
                )
        else:
            # PG: col_ords is already an array.
            idxs_sorted = sorted(
                idxs_in,
                key=lambda i: _index_key(anonymization_key, str(i["name"])),
            )
            for n, i in enumerate(idxs_sorted, start=1):
                col_ords = [int(x) for x in i.get("col_ords") or [] if int(x) > 0]
                idxs_out.append(
                    (
                        f"idx-{n}",
                        {
                            "type": _normalize_index_method(engine, str(i.get("method", "btree"))),
                            "unique": bool(i.get("unique", False)),
                            "cols": col_ords,
                        },
                    )
                )

        blueprint = {
            "rows": rows,
            "table_bytes": table_bytes,
            "index_bytes": index_bytes,
            "schema": schema_to_id.get(str(t["schema_name"]), "schema-?"),
            "has_clustered_index": bool(t.get("has_clustered_index", False)),
            "stats_freshness": "",  # SQL path does not classify freshness; binary path does.
            "cols": dict(cols_out),
            "idxs": dict(idxs_out),
        }
        table_blueprints.append((tid, blueprint))

        named_fk_groups: Dict[Tuple[str, str], List[Tuple[int, int, int]]] = {}
        for fk_index, fk in enumerate(t.get("fks") or []):
            to_oid = fk.get("to_oid")
            if to_oid is None:
                continue
            to_id = oid_to_id.get(to_oid)
            if not to_id:
                raise ValueError(
                    f"foreign key on {t['oid']!r} references an uncollected table {to_oid!r}"
                )
            # PG path: col_ords (array of ints).
            # MySQL and SQL Server paths: one named-column row per FK member.
            if "col_name" in fk:
                child_ordinal = col_name_to_ord.get(str(fk["col_name"]).lower())
                parent_ordinal = column_ordinals_by_oid.get(to_oid, {}).get(
                    str(fk.get("to_col_name", "")).lower()
                )
                if child_ordinal is None or parent_ordinal is None:
                    raise ValueError(
                        f"foreign key on {t['oid']!r} has an unresolved child or parent column"
                    )
                identity = str(
                    fk.get("fk_id")
                    or fk.get("fk_name")
                    or f"legacy-single-column-{fk_index}"
                )
                position = int(fk.get("position", fk_index + 1))
                named_fk_groups.setdefault((to_id, identity), []).append(
                    (position, child_ordinal, parent_ordinal)
                )
            else:
                cols = [int(x) for x in fk.get("col_ords") or [] if int(x) > 0]
                to_cols = [int(x) for x in fk.get("to_col_ords") or [] if int(x) > 0]
                if not cols or len(cols) != len(to_cols):
                    raise ValueError(
                        f"foreign key on {t['oid']!r} has incomplete child/parent ordinals"
                    )
                fk_edges_by_from.setdefault(tid, []).append(
                    {"to": to_id, "cols": cols, "to_cols": to_cols}
                )

        for (to_id, _identity), members in sorted(named_fk_groups.items()):
            ordered = sorted(members)
            fk_edges_by_from.setdefault(tid, []).append(
                {
                    "to": to_id,
                    "cols": [child for _position, child, _parent in ordered],
                    "to_cols": [parent for _position, _child, parent in ordered],
                }
            )

    for k in fk_edges_by_from:
        fk_edges_by_from[k].sort(key=lambda e: (e["to"], e["cols"], e["to_cols"]))

    out = []
    out.append("# dbwarp-blueprint v6\n")
    out.append("# Anonymous database Blueprint. Source object names and row values are excluded.\n")
    out.append("# Review under your organization's data-classification policy before sharing.\n")
    out.append("# https://github.com/DBWarp/dbwarp-blueprint\n\n")
    out.append(
        "# Producer: blueprint_format.py SQL fallback; anonymization key source: "
        f"{anonymization_key_source}\n\n"
    )
    out.append("schema_version = 6\n")
    out.append(f'generated_at = "{generated_at}"\n')
    out.append(f'engine = "{_toml_str_escape(engine)}"\n')
    out.append(f'engine_version = "{_toml_str_escape(engine_version)}"\n')
    out.append(f'source_kind = "{_toml_str_escape(source_kind)}"\n')
    out.append('length_metadata = "not-captured"\n')
    out.append('declared_length_fidelity = "not-captured"\n')
    out.append('index_length_fidelity = "not-captured"\n')
    out.append('observed_length_fidelity = "not-sampled"\n\n')
    out.append("[totals]\n")
    out.append(f"index_bytes = {totals_index_bytes}\n")
    out.append(f"row_count = {totals_row_count}\n")
    out.append(f"table_bytes = {totals_table_bytes}\n")
    out.append(f"table_count = {totals_table_count}\n\n")

    # The SQL fallback has no topology probes. Preserve its useful local
    # catalog estimates, but mark their logical dataset coverage unknown rather
    # than treating one endpoint as proof of a complete cluster.
    row_count_method = {
        "postgresql": "postgres-planner-estimate",
        "mysql": "mysql-table-statistics",
        "sqlserver": "sqlserver-partition-counter",
    }[engine]
    size_method = {
        "postgresql": "postgres-local-relation-size",
        "mysql": "mysql-information-schema",
        "sqlserver": "sqlserver-partition-pages",
    }[engine]
    out.append("[database_topology]\n")
    out.append('contract = "dbwarp-blueprint-topology/v1"\n')
    out.append('deployment = "unknown"\n')
    out.append('local_role = "unknown"\n')
    out.append('visibility = "unknown"\n')
    out.append("member_count = 0\n")
    out.append("identifiers_redacted = true\n\n")
    out.append("[dataset_scope]\n")
    out.append('contract = "dbwarp-blueprint-dataset-scope/v1"\n')
    out.append('layout = "unknown"\n')
    out.append('table_inventory_completeness = "unknown"\n')
    out.append('row_count_completeness = "unknown"\n')
    out.append('size_completeness = "unknown"\n')
    out.append(f'row_count_method = "{row_count_method}"\n')
    out.append(f'size_method = "{size_method}"\n')
    out.append('limitations = ["topology-unobserved", "topology-visibility-unknown"]\n\n')

    for tid, blueprint in table_blueprints:
        out.append(f"[tables.{tid}]\n")
        out.append(f"has_clustered_index = {_toml_bool(blueprint['has_clustered_index'])}\n")
        out.append(f"index_bytes = {blueprint['index_bytes']}\n")
        out.append(f"rows = {blueprint['rows']}\n")
        out.append(f'schema = "{blueprint["schema"]}"\n')
        if blueprint["stats_freshness"]:
            out.append(f'stats_freshness = "{blueprint["stats_freshness"]}"\n')
        out.append(f"table_bytes = {blueprint['table_bytes']}\n")
        out.append("\n")
        for cid, c in sorted(blueprint["cols"].items()):
            out.append(f"[tables.{tid}.cols.{cid}]\n")
            out.append(f"len_avg = {c['len_avg']}\n")
            out.append(f"len_p95 = {c['len_p95']}\n")
            out.append(f"nullable = {_toml_bool(c['nullable'])}\n")
            out.append(f"ordinal = {c['ordinal']}\n")
            if c["style"]:
                out.append(f'style = "{_toml_str_escape(c["style"])}"\n')
            out.append(f'type = "{_toml_str_escape(c["type"])}"\n')
            out.append("\n")
        for iid, idx in sorted(blueprint["idxs"].items()):
            out.append(f"[tables.{tid}.idxs.{iid}]\n")
            out.append(f"cols = {idx['cols']}\n")
            out.append(f'type = "{_toml_str_escape(idx["type"])}"\n')
            out.append(f"unique = {_toml_bool(idx['unique'])}\n")
            out.append("\n")

    if fk_edges_by_from:
        out.append("[fk_edges]\n")
        for src, edges in sorted(fk_edges_by_from.items()):
            edges_repr = ", ".join(
                "{ to = \""
                + e["to"]
                + "\", cols = "
                + repr(list(e["cols"]))
                + ", to_cols = "
                + repr(list(e["to_cols"]))
                + " }"
                for e in edges
            )
            out.append(f'{src} = [{edges_repr}]\n')

    return "".join(out)


def _toml_bool(b: bool) -> str:
    return "true" if b else "false"


def _toml_str_escape(s: str) -> str:
    return (
        s.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\t", "\\t")
    )


if __name__ == "__main__":
    sys.exit(main())

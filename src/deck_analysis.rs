// --- analysis --------------------------------------------------------------
struct CompressionTable<'a> {
    id: &'a str,
    ratio_zstd_3: f64,
    table_bytes: u64,
}

struct CompressionSummary<'a> {
    measured_tables: usize,
    biased_tables: usize,
    sample_rows: u64,
    sample_bytes: u64,
    raw_bytes: u64,
    projected_bytes: u64,
    weighted_ratio_zstd_3: f64,
    projected_reduction_pct: f64,
    top_tables: Vec<CompressionTable<'a>>,
}

struct Deck<'a> {
    name: String,
    version: &'a str,
    source: &'a str,
    generated: &'a str,
    totals: &'a Totals,
    tables: Vec<(&'a str, &'a BlueprintTable)>,
    top_tables: Vec<(&'a str, &'a BlueprintTable)>,
    edges_count: usize,
    indeg_sorted: Vec<(&'a str, u32)>,
    islands: usize,
    type_dist: Vec<(String, u32)>,
    idx_type_dist: Vec<(String, u32)>,
    idx_unique: [u32; 2],
    total_columns: u64,
    total_indexes: u64,
    schemas: usize,
    compression: Option<CompressionSummary<'a>>,
    fk: Option<(&'a str, &'a str, u32)>,
}

fn cols_in_order(t: &BlueprintTable) -> Vec<&crate::format::BlueprintColumn> {
    let mut v: Vec<&crate::format::BlueprintColumn> = t.cols.values().collect();
    v.sort_by_key(|c| c.ordinal);
    v
}

fn idxs_in_order(t: &BlueprintTable) -> Vec<(&str, &crate::format::BlueprintIndex)> {
    let mut v: Vec<(&str, &crate::format::BlueprintIndex)> =
        t.idxs.iter().map(|(k, x)| (k.as_str(), x)).collect();
    v.sort_by_key(|(k, _)| {
        k.rsplit('-')
            .next()
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0)
    });
    v
}

fn analyze(blueprint: &BlueprintFile) -> Deck<'_> {
    let tables: Vec<(&str, &BlueprintTable)> = blueprint
        .tables
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    let mut edges: Vec<(&str, &str, u32)> = Vec::new();
    let mut indeg: BTreeMap<&str, u32> = BTreeMap::new();
    for (src, list) in &blueprint.fk_edges {
        for e in list {
            let col = e.cols.first().copied().unwrap_or(0);
            edges.push((src.as_str(), e.to.as_str(), col));
            *indeg.entry(e.to.as_str()).or_insert(0) += 1;
        }
    }
    let mut involved: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (f, to, _c) in &edges {
        involved.insert(*f);
        involved.insert(*to);
    }
    let mut islands = 0usize;
    for (id, _t) in &tables {
        if !involved.contains(*id) {
            islands += 1;
        }
    }

    let mut type_map: BTreeMap<String, u32> = BTreeMap::new();
    let mut idx_type_map: BTreeMap<String, u32> = BTreeMap::new();
    let mut idx_unique = [0u32; 2];
    let mut total_columns = 0u64;
    let mut total_indexes = 0u64;
    let mut schema_set: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for (_, t) in &tables {
        schema_set.insert(t.schema.as_str());
        for c in t.cols.values() {
            *type_map.entry(c.column_type.clone()).or_insert(0) += 1;
            total_columns += 1;
        }
        for x in t.idxs.values() {
            *idx_type_map.entry(x.index_type.clone()).or_insert(0) += 1;
            idx_unique[if x.unique { 1 } else { 0 }] += 1;
            total_indexes += 1;
        }
    }

    let mut type_dist: Vec<(String, u32)> = type_map.into_iter().collect();
    type_dist.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut idx_type_dist: Vec<(String, u32)> = idx_type_map.into_iter().collect();
    idx_type_dist.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut indeg_sorted: Vec<(&str, u32)> = indeg.into_iter().collect();
    indeg_sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    let mut top_tables = tables
        .iter()
        .copied()
        .filter(|(_, table)| table.counts_toward_totals())
        .collect::<Vec<_>>();
    top_tables.sort_by(|a, b| b.1.table_bytes.cmp(&a.1.table_bytes).then(a.0.cmp(b.0)));

    let mut comp_tables: Vec<CompressionTable<'_>> = Vec::new();
    let mut comp_raw_bytes = 0u64;
    let mut comp_projected_bytes = 0.0f64;
    let mut comp_sample_rows = 0u64;
    let mut comp_sample_bytes = 0u64;
    let mut comp_biased_tables = 0usize;
    for (tid, t) in tables
        .iter()
        .filter(|(_, table)| table.counts_toward_totals())
    {
        if let Some(c) = &t.compression {
            if c.measured && c.ratio_zstd_3.is_finite() && c.ratio_zstd_3 > 0.0 {
                comp_raw_bytes = comp_raw_bytes.saturating_add(t.table_bytes);
                comp_projected_bytes += t.table_bytes as f64 / c.ratio_zstd_3;
                comp_sample_rows = comp_sample_rows.saturating_add(c.sample_rows);
                comp_sample_bytes = comp_sample_bytes.saturating_add(c.sample_bytes);
                if c.sampled_with_bias {
                    comp_biased_tables += 1;
                }
                comp_tables.push(CompressionTable {
                    id: *tid,
                    ratio_zstd_3: c.ratio_zstd_3,
                    table_bytes: t.table_bytes,
                });
            }
        }
    }
    comp_tables.sort_by(|a, b| {
        b.ratio_zstd_3
            .partial_cmp(&a.ratio_zstd_3)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.id.cmp(b.id))
    });
    let compression =
        if comp_raw_bytes > 0 && comp_projected_bytes.is_finite() && comp_projected_bytes > 0.0 {
            let projected = comp_projected_bytes.min(u64::MAX as f64).round() as u64;
            Some(CompressionSummary {
                measured_tables: comp_tables.len(),
                biased_tables: comp_biased_tables,
                sample_rows: comp_sample_rows,
                sample_bytes: comp_sample_bytes,
                raw_bytes: comp_raw_bytes,
                projected_bytes: projected,
                weighted_ratio_zstd_3: comp_raw_bytes as f64 / comp_projected_bytes,
                projected_reduction_pct: (1.0 - (comp_projected_bytes / comp_raw_bytes as f64))
                    * 100.0,
                top_tables: comp_tables,
            })
        } else {
            None
        };

    let fk = edges.first().map(|(f, t, c)| (*f, *t, *c));

    Deck {
        name: engine_name(&blueprint.engine),
        version: &blueprint.engine_version,
        source: &blueprint.source_kind,
        generated: &blueprint.generated_at,
        totals: &blueprint.totals,
        tables,
        top_tables,
        edges_count: edges.len(),
        indeg_sorted,
        islands,
        type_dist,
        idx_type_dist,
        idx_unique,
        total_columns,
        total_indexes,
        schemas: schema_set.len(),
        compression,
        fk,
    }
}

fn col_chips(t: &BlueprintTable) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut cnt: BTreeMap<String, u32> = BTreeMap::new();
    for c in cols_in_order(t) {
        if !cnt.contains_key(&c.column_type) {
            order.push(c.column_type.clone());
        }
        *cnt.entry(c.column_type.clone()).or_insert(0) += 1;
    }
    order
        .into_iter()
        .map(|tp| {
            let n = cnt[&tp];
            if n > 1 {
                format!("{} x{}", tp, n)
            } else {
                tp
            }
        })
        .collect()
}

fn idx_line(name: &str, ix: &crate::format::BlueprintIndex) -> String {
    let cols = ix
        .cols
        .iter()
        .map(|c| format!("col-{}", c))
        .collect::<Vec<_>>()
        .join(", ");
    let uniq = if ix.unique { tr("deck.unique") } else { "" };
    format!("{} {}{} ({})", name, uniq, ix.index_type, cols)
}

fn counted_table_count(d: &Deck<'_>) -> u64 {
    d.top_tables.len() as u64
}

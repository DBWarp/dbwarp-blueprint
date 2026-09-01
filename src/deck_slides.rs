// --- slides ----------------------------------------------------------------
fn title_tagline_runs() -> Vec<Run> {
    let tagline = tr("deck.brand_tagline");
    match tagline.split_once('·') {
        Some((left, right)) => vec![
            run(left.trim_end(), 1450, true, true, TAGLINE_LIGHT, HEAD),
            run(" · ", 1450, true, true, TAGLINE_SEP, HEAD),
            run(right.trim_start(), 1450, true, true, CYAN, HEAD),
        ],
        None => vec![run(tagline, 1450, true, true, TAGLINE_LIGHT, HEAD)],
    }
}
fn build_title(d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(INK));
    s.brand_logo(0.9, 1.15, 0.72, true);
    s.text(
        0.93,
        2.02,
        6.0,
        0.34,
        vec![para("l", title_tagline_runs())],
        "t",
    );
    s.text(
        CONTENT_X,
        3.15,
        CONTENT_W,
        0.72,
        vec![para(
            "l",
            vec![run(tr("deck.report"), 3300, true, false, WHITE, HEAD)],
        )],
        "t",
    );
    s.rect(0.9, 4.25, 2.4, 0.04, Some(CYAN), None, 1.0, None);
    let tc = if d.totals.table_count > 0 {
        d.totals.table_count
    } else {
        counted_table_count(d)
    };
    let sub = trf(
        "deck.title_meta",
        &[
            ("name", d.name.clone()),
            ("version", d.version.to_string()),
            ("source", d.source.to_string()),
            ("tables", table_count_phrase(tc)),
            ("generated", fmt_generated_at_display(d.generated)),
        ],
    );
    s.text(
        0.9,
        4.42,
        11.5,
        0.4,
        vec![para("l", vec![run(sub, 1400, true, false, ICE, HEAD)])],
        "t",
    );
    s
}

fn build_executive(d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(INK));
    kicker_title(
        &mut s,
        tr("deck.executive"),
        tr("deck.executive.subtitle"),
        true,
    );
    let top_count = d.top_tables.len().min(10);
    let top_bytes = d
        .top_tables
        .iter()
        .take(top_count)
        .fold(0u64, |acc, (_, t)| acc.saturating_add(t.table_bytes));
    let top_share = if d.totals.table_bytes > 0 {
        top_bytes as f64 / d.totals.table_bytes as f64
    } else {
        0.0
    };
    let connected = d.tables.len().saturating_sub(d.islands);
    let table_count = d.totals.table_count.max(counted_table_count(d));
    let concentration_body = match count_category(top_count as u64) {
        CountCategory::One => trf(
            "deck.concentration_signal.body.one",
            &[("share", fmt_share_pct(top_share))],
        ),
        CountCategory::Few => trf(
            "deck.concentration_signal.body.few",
            &[
                ("count", top_count.to_string()),
                ("share", fmt_share_pct(top_share)),
            ],
        ),
        CountCategory::Other => trf(
            "deck.concentration_signal.body",
            &[
                ("count", top_count.to_string()),
                ("share", fmt_share_pct(top_share)),
            ],
        ),
    };
    let cards = [
        (
            tr("deck.scale_signal"),
            trf(
                "deck.scale_signal.body",
                &[
                    ("tables", table_count_phrase(table_count)),
                    ("rows", row_count_phrase(d.totals.row_count)),
                    ("data", fmt_bytes(d.totals.table_bytes)),
                    ("schemas", schema_count_phrase(d.schemas)),
                ],
            ),
            CYAN,
        ),
        (tr("deck.concentration_signal"), concentration_body, AQUA),
        (
            tr("deck.relationship_signal"),
            trf(
                "deck.relationship_signal.body",
                &[
                    ("foreign_keys", foreign_key_link_count_phrase(d.edges_count)),
                    ("connected", connected.to_string()),
                    ("total", d.tables.len().to_string()),
                ],
            ),
            CYAN,
        ),
        (
            tr("deck.confidence_signal"),
            tr("deck.confidence_signal.body").to_string(),
            AQUA,
        ),
    ];

    let card_w = 5.55;
    let card_gap = 0.4;
    for (idx, (title, body, accent)) in cards.iter().enumerate() {
        let x = if idx % 2 == 0 {
            CONTENT_X
        } else {
            CONTENT_X + card_w + card_gap
        };
        let y = if idx < 2 { 2.1 } else { 4.05 };
        s.rect(
            x,
            y,
            card_w,
            1.45,
            Some(INK2),
            Some(accent),
            1.0,
            Some(6000),
        );
        s.text(
            x + 0.28,
            y + 0.22,
            card_w - 0.56,
            0.34,
            vec![para("l", vec![run(*title, 1550, true, false, WHITE, HEAD)])],
            "t",
        );
        s.text(
            x + 0.28,
            y + 0.72,
            card_w - 0.56,
            0.55,
            vec![para(
                "l",
                vec![run(body.clone(), 1220, false, false, ICE, BODY_F)],
            )],
            "t",
        );
    }
    s
}

fn build_overview(d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(WHITE));
    kicker_title(
        &mut s,
        tr("deck.overview"),
        tr("deck.overview.subtitle"),
        false,
    );
    let t = d.totals;

    let table_count = t.table_count.max(counted_table_count(d));
    s.rect(
        CONTENT_X,
        OVERVIEW_SCHEMA_ROW_Y,
        CONTENT_W,
        OVERVIEW_SCHEMA_ROW_H,
        Some(GREEN_BG),
        Some(GREEN_LN),
        1.0,
        Some(6000),
    );
    s.text(
        1.18,
        1.69,
        2.0,
        0.22,
        vec![para(
            "l",
            vec![run(
                tr(schema_label_key(d.schemas)).to_uppercase(),
                1050,
                true,
                false,
                CYANDK,
                HEAD,
            )],
        )],
        "t",
    );
    s.text(
        OVERVIEW_SCHEMA_COUNT_X,
        OVERVIEW_SCHEMA_COUNT_Y,
        OVERVIEW_SCHEMA_VALUE_W,
        0.38,
        vec![para(
            "l",
            vec![
                run(d.schemas.to_string(), 2050, true, false, INK, HEAD),
                run(
                    format!(" {}", tr(schema_namespace_key(d.schemas))),
                    1320,
                    false,
                    false,
                    BODY,
                    BODY_F,
                ),
            ],
        )],
        "ctr",
    );

    s.rect(
        CONTENT_X,
        OVERVIEW_PRIMARY_ROW_Y,
        CONTENT_W,
        1.2,
        Some(INK),
        Some(INK),
        1.0,
        Some(6000),
    );
    s.text(
        1.18,
        2.6,
        4.0,
        0.3,
        vec![para(
            "l",
            vec![run(
                tr("deck.primary_sizing_inputs"),
                1150,
                true,
                false,
                CYAN,
                HEAD,
            )],
        )],
        "t",
    );
    let primary = [
        (1.18, 2.7, fmt_bytes(t.table_bytes), tr("deck.table_data")),
        (4.85, 2.2, fmt_rows(t.row_count), tr("deck.rows")),
        (8.25, 2.5, commafy(d.total_columns), tr("deck.columns")),
    ];
    for (x, w, value, label) in primary {
        s.text(
            x,
            2.89,
            w,
            0.48,
            vec![para("l", vec![run(value, 2600, true, false, WHITE, HEAD)])],
            "t",
        );
        s.text(
            x,
            3.34,
            w,
            0.26,
            vec![para("l", vec![run(label, 1100, false, false, ICE, BODY_F)])],
            "t",
        );
    }

    let panel_gap = 0.2;
    let panel_w = (CONTENT_W - panel_gap) / 2.0;
    let lower_y = 3.83;
    let lower_h = 1.8;
    s.rect(
        CONTENT_X,
        lower_y,
        panel_w,
        lower_h,
        Some(PAPER),
        Some(LINE),
        1.0,
        Some(6000),
    );
    s.text(
        1.18,
        4.0,
        2.5,
        0.3,
        vec![para(
            "l",
            vec![run(tr("deck.structure"), 1250, true, false, CYANDK, HEAD)],
        )],
        "t",
    );
    let structure = [
        (
            1.18,
            2.12,
            commafy(table_count),
            table_metric_label(table_count),
            tr("deck.catalog_table_groups"),
        ),
        (
            3.95,
            2.25,
            fmt_avg_per_table(d.total_columns, table_count),
            tr("deck.avg_cols_per_table"),
            tr("deck.column_density"),
        ),
    ];
    for (x, w, value, label, note) in structure {
        s.text(
            x,
            OVERVIEW_METRIC_VALUE_Y,
            w,
            0.4,
            vec![para("l", vec![run(value, 1900, true, false, INK, HEAD)])],
            "t",
        );
        s.text(
            x,
            OVERVIEW_METRIC_LABEL_Y,
            w,
            0.3,
            vec![para("l", vec![run(label, 1200, true, false, INK, BODY_F)])],
            "t",
        );
        s.text(
            x,
            OVERVIEW_METRIC_NOTE_Y,
            w,
            0.4,
            vec![para(
                "l",
                vec![run(note, 1050, false, false, MUTED, BODY_F)],
            )],
            "t",
        );
    }

    let complexity_x = CONTENT_X + panel_w + panel_gap;
    s.rect(
        complexity_x,
        lower_y,
        panel_w,
        lower_h,
        Some(PAPER),
        Some(LINE),
        1.0,
        Some(6000),
    );
    s.text(
        complexity_x + 0.3,
        4.0,
        3.0,
        0.3,
        vec![para(
            "l",
            vec![run(tr("deck.complexity"), 1250, true, false, CYANDK, HEAD)],
        )],
        "t",
    );
    let complexity = [
        (
            complexity_x + 0.3,
            1.45,
            commafy(d.edges_count as u64),
            foreign_key_metric_label(d.edges_count),
            tr("deck.load_order_links"),
            1900,
        ),
        (
            complexity_x + 1.95,
            1.45,
            commafy(d.total_indexes),
            index_metric_label(d.total_indexes),
            tr("deck.secondary_objects"),
            1900,
        ),
        (
            complexity_x + 3.6,
            2.0,
            fmt_bytes(t.index_bytes),
            tr("deck.secondary_structure"),
            tr("deck.index_storage_size"),
            1900,
        ),
    ];
    for (x, w, value, label, note, value_size) in complexity {
        s.text(
            x,
            OVERVIEW_METRIC_VALUE_Y,
            w,
            0.4,
            vec![para(
                "l",
                vec![run(value, value_size, true, false, INK, HEAD)],
            )],
            "t",
        );
        s.text(
            x,
            OVERVIEW_METRIC_LABEL_Y,
            w,
            0.3,
            vec![para("l", vec![run(label, 1200, true, false, INK, BODY_F)])],
            "t",
        );
        if !note.is_empty() {
            s.text(
                x,
                OVERVIEW_METRIC_NOTE_Y,
                w,
                0.4,
                vec![para(
                    "l",
                    vec![run(note, 1050, false, false, MUTED, BODY_F)],
                )],
                "t",
            );
        }
    }

    s.rect(
        CONTENT_X,
        5.82,
        CONTENT_W,
        0.5,
        Some(GREEN_BG),
        Some(GREEN_LN),
        1.0,
        Some(10000),
    );
    s.text(
        1.18,
        5.9,
        10.84,
        0.3,
        vec![para(
            "l",
            vec![
                run(
                    tr("deck.anonymous_prefix"),
                    1250,
                    true,
                    false,
                    CYANDK,
                    BODY_F,
                ),
                run(
                    tr("deck.anonymous_suffix"),
                    1250,
                    false,
                    false,
                    BODY,
                    BODY_F,
                ),
            ],
        )],
        "ctr",
    );
    s
}

fn build_tables(d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(WHITE));
    kicker_title(
        &mut s,
        tr("deck.tables.section"),
        tr("deck.tables.sized"),
        false,
    );
    let maxb = d
        .tables
        .iter()
        .map(|(_, t)| t.table_bytes)
        .max()
        .unwrap_or(1)
        .max(1);
    let compact = d.tables.len() >= 4;
    let row_h = if compact { 1.16 } else { 1.5 };
    let row_gap = if compact { 0.12 } else { 0.22 };
    let mut y = if compact { 2.0 } else { 2.2 };
    for (tid, t) in &d.tables {
        s.rect(
            0.9,
            y,
            11.5,
            row_h,
            Some(PAPER),
            Some(LINE),
            1.0,
            Some(5000),
        );
        s.text(
            1.15,
            y + 0.15,
            6.5,
            0.4,
            vec![para(
                "l",
                vec![
                    run(
                        *tid,
                        if compact { 1700 } else { 2000 },
                        true,
                        false,
                        INK,
                        HEAD,
                    ),
                    run(
                        format!("   \u{b7} {}", t.schema),
                        if compact { 1200 } else { 1400 },
                        false,
                        false,
                        MUTED,
                        BODY_F,
                    ),
                ],
            )],
            "t",
        );
        let clab = if t.has_clustered_index {
            tr("deck.clustered")
        } else {
            tr("deck.heap")
        };
        let meta = trf(
            "deck.table.meta",
            &[
                ("rows", fmt_rows(t.rows)),
                ("columns", t.cols.len().to_string()),
                ("indexes", t.idxs.len().to_string()),
                ("layout", clab.to_string()),
            ],
        );
        s.text(
            6.6,
            y + 0.2,
            5.55,
            0.3,
            vec![para(
                "r",
                vec![run(
                    meta,
                    if compact { 1050 } else { 1200 },
                    false,
                    false,
                    MUTED,
                    BODY_F,
                )],
            )],
            "t",
        );
        let bar_y = y + if compact { 0.54 } else { 0.66 };
        s.rect(
            1.15,
            bar_y,
            11.0,
            0.13,
            Some(TABLE_SIZE_TRACK),
            None,
            1.0,
            Some(20000),
        );
        let frac = t.table_bytes as f64 / maxb as f64;
        s.rect(
            1.15,
            bar_y,
            (11.0 * frac).max(0.08),
            0.13,
            Some(TABLE_SIZE_BAR),
            None,
            1.0,
            Some(20000),
        );
        let chips = col_chips(t).join("   ");
        let sizes = trf(
            "deck.table.sizes",
            &[
                ("data", fmt_bytes(t.table_bytes)),
                ("indexes", fmt_bytes(t.index_bytes)),
            ],
        );
        s.text(
            1.15,
            y + if compact { 0.77 } else { 0.95 },
            11.0,
            0.34,
            vec![para(
                "l",
                vec![
                    run(
                        chips,
                        if compact { 1050 } else { 1200 },
                        false,
                        false,
                        BODY,
                        BODY_F,
                    ),
                    run(
                        format!("      {}", sizes),
                        if compact { 1050 } else { 1200 },
                        false,
                        false,
                        MUTED,
                        BODY_F,
                    ),
                ],
            )],
            "t",
        );
        y += row_h + row_gap;
    }
    s
}

fn build_largest(d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(WHITE));
    kicker_title(&mut s, tr("deck.tables.section"), tr("deck.largest"), false);
    let show: Vec<(&str, &BlueprintTable)> = d.top_tables.iter().take(10).cloned().collect();
    let maxb = show
        .iter()
        .map(|(_, t)| t.table_bytes)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut y = 1.95;
    for (tid, t) in &show {
        s.text(
            0.9,
            y,
            2.4,
            0.32,
            vec![para(
                "l",
                vec![
                    run(*tid, 1300, true, false, INK, HEAD),
                    run(format!("  {}", t.schema), 1000, false, false, MUTED, BODY_F),
                ],
            )],
            "ctr",
        );
        s.rect(
            3.4,
            y + 0.10,
            6.0,
            0.12,
            Some(TABLE_SIZE_TRACK),
            None,
            1.0,
            Some(20000),
        );
        let frac = t.table_bytes as f64 / maxb as f64;
        s.rect(
            3.4,
            y + 0.10,
            (6.0 * frac).max(0.06),
            0.12,
            Some(TABLE_SIZE_BAR),
            None,
            1.0,
            Some(20000),
        );
        s.text(
            9.5,
            y,
            2.9,
            0.32,
            vec![para(
                "r",
                vec![run(
                    trf(
                        "deck.row_and_bytes",
                        &[
                            ("rows", fmt_rows(t.rows)),
                            ("bytes", fmt_bytes(t.table_bytes)),
                        ],
                    ),
                    1100,
                    false,
                    false,
                    BODY,
                    BODY_F,
                )],
            )],
            "ctr",
        );
        y += 0.42;
    }
    let total = d.tables.len();
    if total > show.len() {
        let shown: u64 = show.iter().map(|(_, t)| t.table_bytes).sum();
        let rest = d.totals.table_bytes.saturating_sub(shown);
        s.text(
            0.9,
            y + 0.1,
            11.5,
            0.32,
            vec![para(
                "l",
                vec![run(
                    trf(
                        "deck.more_tables",
                        &[
                            ("count", (total - show.len()).to_string()),
                            ("bytes", fmt_bytes(rest)),
                        ],
                    ),
                    1200,
                    false,
                    false,
                    MUTED,
                    BODY_F,
                )],
            )],
            "t",
        );
    }
    s
}

fn build_composition(d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(WHITE));
    kicker_title(
        &mut s,
        tr("deck.composition"),
        tr("deck.composition.subtitle"),
        false,
    );
    s.text(
        0.9,
        1.95,
        5.6,
        0.3,
        vec![para(
            "l",
            vec![run(
                tr("deck.columns_by_type"),
                1500,
                true,
                false,
                INK,
                HEAD,
            )],
        )],
        "t",
    );
    let maxc = d
        .type_dist
        .iter()
        .take(8)
        .map(|(_, c)| *c)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut y = 2.5;
    for (typ, cnt) in d.type_dist.iter().take(8) {
        s.text(
            0.9,
            y,
            2.1,
            0.28,
            vec![para(
                "l",
                vec![run(typ.clone(), 1100, false, false, BODY, BODY_F)],
            )],
            "ctr",
        );
        s.rect(3.1, y + 0.06, 2.4, 0.12, Some(PGLT), None, 1.0, Some(20000));
        s.rect(
            3.1,
            y + 0.06,
            (2.4 * (*cnt as f64) / (maxc as f64)).max(0.05),
            0.12,
            Some(CYANDK),
            None,
            1.0,
            Some(20000),
        );
        s.text(
            5.6,
            y,
            0.9,
            0.28,
            vec![para(
                "l",
                vec![run(commafy(*cnt as u64), 1100, true, false, INK, BODY_F)],
            )],
            "ctr",
        );
        y += 0.42;
    }
    s.text(
        7.0,
        1.95,
        5.4,
        0.3,
        vec![para(
            "l",
            vec![run(tr("deck.indexes_totals"), 1500, true, false, INK, HEAD)],
        )],
        "t",
    );
    let nu = d.idx_unique;
    let lines = [
        (
            tr("deck.columns_total").to_string(),
            commafy(d.total_columns),
        ),
        (
            tr("deck.indexes_total").to_string(),
            commafy(d.total_indexes),
        ),
        (
            tr("deck.unique_nonunique").to_string(),
            format!("{} / {}", commafy(nu[1] as u64), commafy(nu[0] as u64)),
        ),
        (tr("deck.schemas").to_string(), d.schemas.to_string()),
    ];
    let mut yy = 2.5;
    for (lab, val) in lines.iter() {
        s.text(
            7.0,
            yy,
            3.3,
            0.3,
            vec![para(
                "l",
                vec![run(lab.clone(), 1200, false, false, MUTED, BODY_F)],
            )],
            "ctr",
        );
        s.text(
            10.3,
            yy,
            2.1,
            0.3,
            vec![para(
                "r",
                vec![run(val.clone(), 1300, true, false, INK, HEAD)],
            )],
            "ctr",
        );
        yy += 0.5;
    }
    let chip = d
        .idx_type_dist
        .iter()
        .take(5)
        .map(|(k, v)| format!("{} {}", k, commafy(*v as u64)))
        .collect::<Vec<_>>()
        .join("   ");
    s.text(
        7.0,
        yy + 0.1,
        5.4,
        0.5,
        vec![para(
            "l",
            vec![
                run(tr("deck.index_methods"), 1100, false, false, MUTED, BODY_F),
                run(chip, 1100, false, false, BODY, BODY_F),
            ],
        )],
        "t",
    );
    s
}

fn find_table<'a>(
    tables: &[(&'a str, &'a BlueprintTable)],
    id: &str,
) -> Option<&'a BlueprintTable> {
    tables.iter().find(|&&(k, _)| k == id).map(|&(_, t)| t)
}

fn schema_box(s: &mut SlideB, x: f64, accent: &str, tid: &str, t: Option<&BlueprintTable>) {
    s.rect(
        x,
        3.1,
        4.3,
        1.95,
        Some(PAPER),
        Some(accent),
        1.5,
        Some(8000),
    );
    s.text(
        x + 0.28,
        3.28,
        3.7,
        0.4,
        vec![para(
            "l",
            vec![run(tid.to_string(), 1900, true, false, INK, HEAD)],
        )],
        "t",
    );
    if let Some(t) = t {
        let cl = if t.has_clustered_index {
            tr("deck.clustered")
        } else {
            tr("deck.heap")
        };
        s.text(
            x + 0.28,
            3.72,
            3.7,
            0.3,
            vec![para(
                "l",
                vec![run(
                    trf(
                        "deck.schema_table_meta",
                        &[("rows", fmt_rows(t.rows)), ("layout", cl.to_string())],
                    ),
                    1200,
                    false,
                    false,
                    BODY,
                    BODY_F,
                )],
            )],
            "t",
        );
        let mut paras: Vec<Para> = Vec::new();
        for (nm, ix) in idxs_in_order(t).into_iter().take(2) {
            paras.push(Para {
                align: "l",
                space_before: 3,
                runs: vec![run(idx_line(nm, ix), 1100, false, false, CYANDK, BODY_F)],
            });
        }
        if !paras.is_empty() {
            s.text(x + 0.28, 4.05, 3.8, 0.9, paras, "t");
        }
    }
}

fn build_schema(d: &Deck, fk: (&str, &str, u32)) -> SlideB {
    let mut s = SlideB::new(Some(WHITE));
    kicker_title(
        &mut s,
        tr("deck.schema_map"),
        tr("deck.foreign_key_relationships"),
        false,
    );
    let child = find_table(&d.tables, fk.0);
    let parent = find_table(&d.tables, fk.1);
    schema_box(&mut s, CONTENT_X, CYANDK, fk.0, child);
    s.text(
        5.45,
        3.45,
        2.45,
        0.3,
        vec![para(
            "ctr",
            vec![run(
                format!("fk \u{b7} col-{}", fk.2),
                1200,
                false,
                false,
                CYANDK,
                BODY_F,
            )],
        )],
        "t",
    );
    s.arrow(5.55, 3.92, 2.25, 0.38, CYANDK);
    schema_box(&mut s, CONTENT_R - 4.3, PG, fk.1, parent);
    s
}

fn build_relationships(d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(WHITE));
    kicker_title(
        &mut s,
        tr("deck.relationships"),
        tr("deck.foreign_key_relationships"),
        false,
    );
    s.text(
        0.9,
        2.3,
        5.3,
        1.1,
        vec![para(
            "l",
            vec![run(
                commafy(d.edges_count as u64),
                6600,
                true,
                false,
                CYANDK,
                HEAD,
            )],
        )],
        "t",
    );
    s.text(
        0.95,
        3.55,
        5.3,
        0.4,
        vec![para(
            "l",
            vec![run(tr("deck.foreign_keys"), 1600, false, false, INK, HEAD)],
        )],
        "t",
    );
    let involved = d.tables.len() - d.islands;
    s.text(
        0.95,
        4.25,
        5.7,
        0.4,
        vec![para(
            "l",
            vec![run(
                trf(
                    "deck.connected",
                    &[
                        ("connected", involved.to_string()),
                        ("total", d.tables.len().to_string()),
                        ("standalone", d.islands.to_string()),
                    ],
                ),
                1300,
                false,
                false,
                BODY,
                BODY_F,
            )],
        )],
        "t",
    );
    s.text(
        7.2,
        2.05,
        5.2,
        0.3,
        vec![para(
            "l",
            vec![run(
                tr("deck.most_referenced"),
                1400,
                true,
                false,
                INK,
                HEAD,
            )],
        )],
        "t",
    );
    let mut yy = 2.65;
    for (tid, cnt) in d.indeg_sorted.iter().take(6) {
        s.text(
            7.2,
            yy,
            3.0,
            0.3,
            vec![para(
                "l",
                vec![run(*tid, 1300, false, false, CYANDK, BODY_F)],
            )],
            "ctr",
        );
        s.text(
            10.2,
            yy,
            2.2,
            0.3,
            vec![para(
                "r",
                vec![run(
                    trf("deck.refs", &[("count", cnt.to_string())]),
                    1300,
                    true,
                    false,
                    BODY,
                    BODY_F,
                )],
            )],
            "ctr",
        );
        yy += 0.46;
    }
    s
}

fn build_compression(d: &Deck) -> Option<SlideB> {
    let c = d.compression.as_ref()?;
    let mut s = SlideB::new(Some(WHITE));
    kicker_title(
        &mut s,
        tr("deck.compression"),
        tr("deck.compression.subtitle"),
        false,
    );

    let tiles = [
        (
            tr("deck.sampled_tables").to_string(),
            c.measured_tables.to_string(),
        ),
        (
            tr("deck.weighted_zstd3").to_string(),
            fmt_ratio(c.weighted_ratio_zstd_3),
        ),
        (
            tr("deck.projected_compressed").to_string(),
            fmt_bytes(c.projected_bytes),
        ),
        (
            tr("deck.projected_reduction").to_string(),
            fmt_pct(c.projected_reduction_pct),
        ),
    ];
    let (gap, x0, y0) = (0.2, CONTENT_X, 1.9);
    let tw = (CONTENT_W - 3.0 * gap) / 4.0;
    for (k, (lab, val)) in tiles.iter().enumerate() {
        let x = x0 + (k as f64) * (tw + gap);
        s.rect(x, y0, tw, 1.15, Some(PAPER), Some(LINE), 1.0, Some(6000));
        s.text(
            x + 0.22,
            y0 + 0.16,
            tw - 0.4,
            0.3,
            vec![para(
                "l",
                vec![run(lab.clone(), 1200, false, false, MUTED, BODY_F)],
            )],
            "t",
        );
        s.text(
            x + 0.22,
            y0 + 0.48,
            tw - 0.4,
            0.5,
            vec![para(
                "l",
                vec![run(val.clone(), 2400, true, false, INK, HEAD)],
            )],
            "t",
        );
    }

    let sample = trf(
        "deck.sample_meta",
        &[
            ("rows", commafy(c.sample_rows)),
            ("sample_bytes", fmt_bytes(c.sample_bytes)),
            ("raw_bytes", fmt_bytes(c.raw_bytes)),
            ("biased", c.biased_tables.to_string()),
        ],
    );
    s.text(
        0.9,
        3.35,
        11.5,
        0.35,
        vec![para(
            "l",
            vec![run(sample, 1250, false, false, MUTED, BODY_F)],
        )],
        "t",
    );

    s.text(
        0.9,
        4.0,
        5.2,
        0.3,
        vec![para(
            "l",
            vec![run(
                tr("deck.most_compressible"),
                1500,
                true,
                false,
                INK,
                HEAD,
            )],
        )],
        "t",
    );
    let top = c.top_tables.iter().take(6).collect::<Vec<_>>();
    let maxr = top
        .iter()
        .map(|t| t.ratio_zstd_3)
        .fold(0.0f64, f64::max)
        .max(1.0);
    let mut y = 4.48;
    for t in top {
        s.text(
            0.9,
            y,
            2.2,
            0.28,
            vec![para(
                "l",
                vec![run(t.id.to_string(), 1150, true, false, BODY, BODY_F)],
            )],
            "ctr",
        );
        s.rect(3.2, y + 0.07, 2.8, 0.12, Some(PGLT), None, 1.0, Some(20000));
        s.rect(
            3.2,
            y + 0.07,
            (2.8 * t.ratio_zstd_3 / maxr).max(0.05),
            0.12,
            Some(CYANDK),
            None,
            1.0,
            Some(20000),
        );
        s.text(
            6.12,
            y,
            1.1,
            0.28,
            vec![para(
                "r",
                vec![run(
                    fmt_ratio(t.ratio_zstd_3),
                    1150,
                    true,
                    false,
                    INK,
                    BODY_F,
                )],
            )],
            "ctr",
        );
        s.text(
            7.55,
            y,
            2.5,
            0.28,
            vec![para(
                "r",
                vec![run(
                    fmt_bytes(t.table_bytes),
                    1050,
                    false,
                    false,
                    MUTED,
                    BODY_F,
                )],
            )],
            "ctr",
        );
        y += 0.38;
    }
    s.text(
        6.15,
        4.12,
        1.1,
        0.22,
        vec![para(
            "r",
            vec![run("zstd-3", 900, false, false, MUTED, BODY_F)],
        )],
        "t",
    );
    s.text(
        7.55,
        4.12,
        2.5,
        0.22,
        vec![para(
            "r",
            vec![run(tr("deck.table_data"), 900, false, false, MUTED, BODY_F)],
        )],
        "t",
    );

    s.rect(
        9.2,
        4.0,
        3.2,
        1.15,
        Some(GREEN_BG),
        Some(GREEN_LN),
        1.0,
        Some(8000),
    );
    s.text(
        9.42,
        4.18,
        2.75,
        0.75,
        vec![para(
            "l",
            vec![run(
                tr("deck.projection"),
                1050,
                false,
                false,
                CYANDK,
                BODY_F,
            )],
        )],
        "t",
    );
    s.text(
        CONTENT_X,
        std::f64::consts::TAU,
        CONTENT_W,
        0.3,
        vec![para(
            "l",
            vec![run(
                tr("deck.sample_disposal"),
                1000,
                false,
                false,
                MUTED,
                BODY_F,
            )],
        )],
        "t",
    );
    Some(s)
}

fn build_ethos(_d: &Deck) -> SlideB {
    let mut s = SlideB::new(Some(INK));
    kicker_title(
        &mut s,
        tr("deck.trust_model"),
        tr("deck.trust.subtitle"),
        true,
    );
    let cards = [
        (tr("deck.no_phone_home"), tr("deck.no_phone_home.body")),
        (tr("deck.one_leak"), tr("deck.one_leak.body")),
        (tr("deck.no_hidden"), tr("deck.no_hidden.body")),
    ];
    let (gap, x0, y0) = (0.27, CONTENT_X, 2.35);
    let cw = (CONTENT_W - 2.0 * gap) / 3.0;
    for (k, (title, body)) in cards.iter().enumerate() {
        let x = x0 + (k as f64) * (cw + gap);
        s.rect(x, y0, cw, 2.65, Some(INK2), Some(CYAN), 1.0, Some(6000));
        s.text(
            x + 0.3,
            y0 + 0.32,
            cw - 0.6,
            0.55,
            vec![para("l", vec![run(*title, 1700, true, false, WHITE, HEAD)])],
            "t",
        );
        s.text(
            x + 0.3,
            y0 + 1.0,
            cw - 0.6,
            1.48,
            vec![para("l", vec![run(*body, 1050, false, false, ICE, BODY_F)])],
            "t",
        );
    }
    s.text(
        CONTENT_X,
        5.4,
        CONTENT_W,
        0.4,
        vec![para(
            "l",
            vec![run(tr("deck.footer"), 1250, false, false, CYAN_LT, BODY_F)],
        )],
        "t",
    );
    s
}

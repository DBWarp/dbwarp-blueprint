// --- speaker notes ---------------------------------------------------------
// Notes interpret the slide rather than repeat its visible labels. Each page
// contains a short natural-language summary derived from the same Blueprint.

#[derive(Clone, Copy, PartialEq, Eq)]
enum NoteStyle {
    Section,
    Body,
}

struct NoteParagraph {
    style: NoteStyle,
    text: String,
}

fn note_section(key: &str) -> NoteParagraph {
    NoteParagraph {
        style: NoteStyle::Section,
        text: tr(key).to_string(),
    }
}

fn note_body(text: impl Into<String>) -> NoteParagraph {
    NoteParagraph {
        style: NoteStyle::Body,
        text: text.into(),
    }
}

fn speaker_notes(summary: Vec<String>) -> Vec<NoteParagraph> {
    let mut notes = vec![note_section("deck.notes.section.summary")];
    notes.extend(
        summary
            .into_iter()
            .filter(|text| !text.is_empty())
            .map(note_body),
    );
    notes
}

fn title_speaker_notes(d: &Deck<'_>) -> Vec<NoteParagraph> {
    let table_count = d.totals.table_count.max(counted_table_count(d));
    speaker_notes(vec![trf(
            "deck.notes.title.summary",
            &[
                ("engine", format!("{} {}", d.name, d.version)),
                ("source", d.source.to_string()),
                ("tables", table_count_phrase(table_count)),
                ("generated", fmt_generated_at_display(d.generated)),
            ],
        )])
}

fn executive_speaker_notes(d: &Deck<'_>) -> Vec<NoteParagraph> {
    let table_count = d.totals.table_count.max(counted_table_count(d));
    let estate_summary = if let Some((largest_id, largest)) = d.top_tables.first() {
        let largest_share = if d.totals.table_bytes > 0 {
            largest.table_bytes as f64 / d.totals.table_bytes as f64
        } else {
            0.0
        };
        trf(
            "deck.notes.executive.summary",
            &[
                ("tables", table_count_phrase(table_count)),
                ("schemas", schema_count_phrase(d.schemas)),
                ("rows", row_count_phrase(d.totals.row_count)),
                ("data", fmt_bytes(d.totals.table_bytes)),
                ("largest", (*largest_id).to_string()),
                ("share", fmt_share_pct(largest_share)),
            ],
        )
    } else {
        trf(
            "deck.notes.executive.summary.empty",
            &[("schemas", schema_count_phrase(d.schemas))],
        )
    };
    let connected = d.tables.len().saturating_sub(d.islands);
    let relationships = if d.edges_count == 1 {
        trf(
            "deck.notes.executive.relationships.one",
            &[
                ("connected", connected.to_string()),
                ("total", d.tables.len().to_string()),
            ],
        )
    } else {
        trf(
            "deck.notes.executive.relationships.other",
            &[
                ("foreign_keys", d.edges_count.to_string()),
                ("connected", connected.to_string()),
                ("total", d.tables.len().to_string()),
            ],
        )
    };
    speaker_notes(vec![estate_summary, relationships])
}

fn overview_speaker_notes(d: &Deck<'_>) -> Vec<NoteParagraph> {
    let table_count = d.totals.table_count.max(counted_table_count(d));
    speaker_notes(vec![trf(
            "deck.notes.overview.summary",
            &[
                ("tables", table_count_phrase(table_count)),
                ("schemas", schema_count_phrase(d.schemas)),
                ("rows", row_count_phrase(d.totals.row_count)),
                ("data", fmt_bytes(d.totals.table_bytes)),
                ("columns", commafy(d.total_columns)),
                ("indexes", commafy(d.total_indexes)),
                (
                    "relationships",
                    foreign_key_link_count_phrase(d.edges_count),
                ),
            ],
        )])
}

fn artifact_inventory_speaker_notes(inventory: &ArtifactInventory) -> Vec<NoteParagraph> {
    if inventory.object_count == 0 {
        return speaker_notes(vec![tr("deck.notes.artifacts.empty").to_string()]);
    }
    let groups = artifact_group_summaries(inventory);
    let largest = groups
        .iter()
        .max_by_key(|group| group.count)
        .expect("the artifact taxonomy is not empty");
    speaker_notes(vec![trf(
        "deck.notes.artifacts.summary",
        &[
            ("objects", commafy(inventory.object_count)),
            ("group", tr(largest.label_key).to_string()),
            ("count", commafy(largest.count)),
        ],
    )])
}

fn complexity_speaker_notes(complexity: &ArtifactComplexity) -> Vec<NoteParagraph> {
    let summary = match complexity.overall_band.as_str() {
        "unknown" => trf(
                "deck.notes.complexity.unknown",
                &[
                    ("eligible", commafy(complexity.eligible_object_count)),
                    ("fully", commafy(complexity.fully_assessed_object_count)),
                ],
            ),
        "not-applicable" => tr("deck.notes.complexity.not_applicable").to_string(),
        _ => trf(
                "deck.notes.complexity.definitive",
                &[
                    (
                        "band",
                        complexity_band_label(&complexity.overall_band).to_string(),
                    ),
                    ("eligible", commafy(complexity.eligible_object_count)),
                ],
            ),
    };
    speaker_notes(vec![summary, tr("deck.notes.complexity.dimensions").to_string()])
}

fn tables_speaker_notes(d: &Deck<'_>) -> Vec<NoteParagraph> {
    let largest = d.tables.iter().max_by_key(|(_, table)| table.table_bytes);
    let summary = match largest {
        Some((table_id, table)) if d.tables.len() == 1 => trf(
            "deck.notes.tables.summary.one",
            &[
                ("data", fmt_bytes(d.totals.table_bytes)),
                ("largest", (*table_id).to_string()),
                ("rows", row_count_phrase(table.rows)),
            ],
        ),
        Some((table_id, table)) => trf(
            "deck.notes.tables.summary.other",
            &[
                ("count", d.tables.len().to_string()),
                ("data", fmt_bytes(d.totals.table_bytes)),
                ("largest", (*table_id).to_string()),
                ("largest_data", fmt_bytes(table.table_bytes)),
                ("rows", row_count_phrase(table.rows)),
                (
                    "remainder",
                    fmt_bytes(d.totals.table_bytes.saturating_sub(table.table_bytes)),
                ),
            ],
        ),
        None => String::new(),
    };
    speaker_notes(vec![summary])
}

fn largest_speaker_notes(d: &Deck<'_>) -> Vec<NoteParagraph> {
    let shown = d.top_tables.iter().take(10).collect::<Vec<_>>();
    let shown_bytes = shown.iter().fold(0u64, |sum, (_, table)| {
        sum.saturating_add(table.table_bytes)
    });
    let shown_share = if d.totals.table_bytes > 0 {
        shown_bytes as f64 / d.totals.table_bytes as f64
    } else {
        0.0
    };
    let summary = shown.first().map_or_else(String::new, |(table_id, table)| {
        trf(
            "deck.notes.largest.summary",
            &[
                ("shown", shown.len().to_string()),
                ("share", fmt_share_pct(shown_share)),
                ("total", fmt_bytes(d.totals.table_bytes)),
                ("largest", (*table_id).to_string()),
                ("largest_data", fmt_bytes(table.table_bytes)),
                ("rows", row_count_phrase(table.rows)),
                (
                    "remainder",
                    fmt_bytes(d.totals.table_bytes.saturating_sub(shown_bytes)),
                ),
            ],
        )
    });
    speaker_notes(vec![summary])
}

fn composition_speaker_notes(d: &Deck<'_>) -> Vec<NoteParagraph> {
    let mut summary = vec![trf(
        "deck.notes.composition.summary",
        &[
            ("columns", commafy(d.total_columns)),
            ("indexes", commafy(d.total_indexes)),
            ("schemas", schema_count_phrase(d.schemas)),
        ],
    )];
    let leading = match (d.type_dist.first(), d.idx_type_dist.first()) {
        (Some((column_type, column_count)), Some((index_method, index_count))) => Some(trf(
            "deck.notes.composition.leading_both",
            &[
                ("column_type", (*column_type).to_string()),
                ("column_count", commafy(u64::from(*column_count))),
                ("index_method", (*index_method).to_string()),
                ("index_count", commafy(u64::from(*index_count))),
            ],
        )),
        (Some((column_type, column_count)), None) => Some(trf(
            "deck.notes.composition.leading_type",
            &[
                ("column_type", (*column_type).to_string()),
                ("column_count", commafy(u64::from(*column_count))),
            ],
        )),
        (None, Some((index_method, index_count))) => Some(trf(
            "deck.notes.composition.leading_index",
            &[
                ("index_method", (*index_method).to_string()),
                ("index_count", commafy(u64::from(*index_count))),
            ],
        )),
        (None, None) => None,
    };
    summary.extend(leading);
    speaker_notes(summary)
}

fn schema_speaker_notes(d: &Deck<'_>, fk: (&str, &str, u32)) -> Vec<NoteParagraph> {
    let mut summary = vec![trf(
        "deck.notes.schema.summary",
        &[
            ("child", fk.0.to_string()),
            ("parent", fk.1.to_string()),
            ("column", fk.2.to_string()),
        ],
    )];
    if let (Some(child), Some(parent)) = (find_table(&d.tables, fk.0), find_table(&d.tables, fk.1))
    {
        summary.push(trf(
            "deck.notes.schema.scale",
            &[
                ("child_rows", row_count_phrase(child.rows)),
                ("parent_rows", row_count_phrase(parent.rows)),
            ],
        ));
    }
    speaker_notes(summary)
}

fn relationships_speaker_notes(d: &Deck<'_>) -> Vec<NoteParagraph> {
    let involved = d.tables.len().saturating_sub(d.islands);
    let mut summary = vec![trf(
        "deck.notes.relationships.summary",
        &[
            ("links", foreign_key_link_count_phrase(d.edges_count)),
            ("connected", involved.to_string()),
            ("total", d.tables.len().to_string()),
            ("standalone", d.islands.to_string()),
        ],
    )];
    if let Some((table_id, references)) = d.indeg_sorted.first() {
        summary.push(trf(
            "deck.notes.relationships.leading",
            &[
                ("table", (*table_id).to_string()),
                ("references", references.to_string()),
            ],
        ));
    }
    speaker_notes(summary)
}

fn compression_speaker_notes(compression: &CompressionSummary<'_>) -> Vec<NoteParagraph> {
    speaker_notes(vec![trf(
            "deck.notes.compression.summary",
            &[
                ("tables", compression.measured_tables.to_string()),
                ("rows", commafy(compression.sample_rows)),
                ("ratio", fmt_ratio(compression.weighted_ratio_zstd_3)),
                ("raw", fmt_bytes(compression.raw_bytes)),
                ("projected", fmt_bytes(compression.projected_bytes)),
                ("reduction", fmt_pct(compression.projected_reduction_pct)),
            ],
        )])
}

fn trust_speaker_notes() -> Vec<NoteParagraph> {
    speaker_notes(vec![tr("deck.notes.trust.summary").to_string()])
}

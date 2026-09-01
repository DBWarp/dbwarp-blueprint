#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{
        BlueprintColumn, BlueprintCompression, BlueprintFile, BlueprintIndex, BlueprintTable,
        FkEdge, Totals, SCHEMA_VERSION,
    };
    use std::collections::BTreeMap;

    fn table(
        rows: u64,
        tb: u64,
        ib: u64,
        clustered: bool,
        cols: &[(&str, u32)],
        idxs: &[(&str, bool, Vec<u32>)],
    ) -> BlueprintTable {
        let mut cm = BTreeMap::new();
        for (ty, ord) in cols {
            cm.insert(
                format!("col-{}", ord),
                BlueprintColumn {
                    ordinal: *ord,
                    column_type: ty.to_string(),
                    nullable: false,
                    len_avg: 10,
                    len_p95: 0,
                    style: String::new(),
                    compression: None,
                    ..BlueprintColumn::default()
                },
            );
        }
        let mut im = BTreeMap::new();
        for (i, (ty, uniq, c)) in idxs.iter().enumerate() {
            im.insert(
                format!("idx-{}", i + 1),
                BlueprintIndex {
                    index_type: ty.to_string(),
                    primary: false,
                    unique: *uniq,
                    cols: c.clone(),
                    include_cols: Vec::new(),
                    expression: false,
                    filtered: false,
                    descending: false,
                    ..BlueprintIndex::default()
                },
            );
        }
        BlueprintTable {
            rows,
            table_bytes: tb,
            index_bytes: ib,
            schema: "schema-A".into(),
            has_clustered_index: clustered,
            stats_freshness: String::new(),
            cols: cm,
            idxs: im,
            compression: None,
            ..BlueprintTable::default()
        }
    }

    fn sample() -> BlueprintFile {
        let mut tables = BTreeMap::new();
        tables.insert(
            "table-001".to_string(),
            table(
                12_500_000,
                4_194_304_000,
                1_048_576_000,
                false,
                &[("bigint", 1), ("text", 2), ("timestamp", 3)],
                &[("btree", true, vec![2]), ("btree", true, vec![1])],
            ),
        );
        tables.insert(
            "table-002".to_string(),
            table(
                985_000,
                319_815_680,
                95_420_416,
                false,
                &[("bigint", 1), ("bigint", 2), ("timestamp", 3)],
                &[("btree", true, vec![1])],
            ),
        );
        let mut fk = BTreeMap::new();
        fk.insert(
            "table-002".to_string(),
            vec![FkEdge {
                to: "table-001".into(),
                cols: vec![2],
                to_cols: vec![1],
                ..FkEdge::default()
            }],
        );
        BlueprintFile {
            artifact_inventory: None,
            schema_version: SCHEMA_VERSION,
            generated_at: "2026-04-28T00:00:00Z".into(),
            engine: "postgresql".into(),
            engine_version: "16.2".into(),
            source_kind: "production".into(),
            length_metadata: "not-captured".into(),
            declared_length_fidelity: "not-captured".into(),
            index_length_fidelity: "not-captured".into(),
            observed_length_fidelity: "not-sampled".into(),
            network: None,
            database_topology: Some(crate::format::DatabaseTopology::unknown()),
            dataset_scope: Some(crate::format::DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            totals: Totals {
                table_count: 2,
                row_count: 13_485_000,
                table_bytes: 4_514_119_680,
                index_bytes: 1_143_996_416,
            },
            tables,
            fk_edges: fk,
        }
    }

    fn parse_blueprint(name: &str, src: &str) -> BlueprintFile {
        toml::from_str(src).unwrap_or_else(|e| panic!("{name} must parse as BlueprintFile: {e}"))
    }

    fn add_compression(sf: &mut BlueprintFile, table_id: &str, ratio3: f64, biased: bool) {
        sf.tables.get_mut(table_id).unwrap().compression = Some(BlueprintCompression {
            measured: true,
            sample_rows: 1000,
            sample_bytes: 1_048_576,
            sample_method: "LIMIT 1000".to_string(),
            sampled_with_bias: biased,
            bias_reason: if biased {
                "unordered_limit_after_empty_TABLESAMPLE".to_string()
            } else {
                String::new()
            },
            ratio_zstd_3: ratio3,
            ratio_stddev: 0.1,
            sample_encoding: "dbwarp-blueprint-rowframe-v1".to_string(),
            ..BlueprintCompression::default()
        });
    }

    fn checked_in_blueprint_cases() -> Vec<(&'static str, BlueprintFile)> {
        vec![
            (
                "pg-small",
                parse_blueprint(
                    "pg-small",
                    include_str!("../tests/fixtures/blueprint_format/pg_expected.toml"),
                ),
            ),
            (
                "mysql-small",
                parse_blueprint(
                    "mysql-small",
                    include_str!("../tests/fixtures/blueprint_format/mysql_expected.toml"),
                ),
            ),
            (
                "mssql-small",
                parse_blueprint(
                    "mssql-small",
                    include_str!("../tests/fixtures/blueprint_format/mssql_expected.toml"),
                ),
            ),
            (
                "saas-medium",
                parse_blueprint("saas-medium", include_str!("../samples/saas-medium.toml")),
            ),
            (
                "ecommerce-large",
                parse_blueprint(
                    "ecommerce-large",
                    include_str!("../samples/ecommerce-large.toml"),
                ),
            ),
            (
                "erp-enterprise",
                parse_blueprint(
                    "erp-enterprise",
                    include_str!("../samples/erp-enterprise.toml"),
                ),
            ),
        ]
    }

    #[test]
    fn produces_zip_with_expected_parts() {
        let b = build_pptx(&sample());
        assert_eq!(&b[..2], b"PK", "must be a zip");
        let s = String::from_utf8_lossy(&b);
        assert!(s.contains("[Content_Types].xml"));
        assert!(s.contains("ppt/presentation.xml"));
        assert!(
            s.contains("ppt/slides/slide6.xml"),
            "small schema => 6 slides incl. executive summary and ethos"
        );
        assert!(!s.contains("ppt/slides/slide7.xml"));
    }

    #[test]
    fn pptx_package_structure_is_self_consistent() {
        let parts = zip_parts(&build_pptx(&sample()));
        assert!(parts.contains_key("[Content_Types].xml"));
        assert!(parts.contains_key("_rels/.rels"));
        assert!(parts.contains_key("ppt/media/dbwarp-logo-dark.png"));
        assert!(parts.contains_key("ppt/media/dbwarp-logo-light.png"));
        assert!(parts.contains_key("ppt/media/dbwarp-logo-dark-small.png"));
        assert!(parts.contains_key("ppt/media/dbwarp-logo-light-small.png"));

        let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
        for part_name in attr_values(&content_types, "PartName") {
            let part_name = part_name.trim_start_matches('/');
            assert!(
                parts.contains_key(part_name),
                "content-types override references missing part {part_name}"
            );
        }

        for (part, bytes) in &parts {
            if !part.ends_with(".rels") {
                continue;
            }
            let xml = String::from_utf8(bytes.clone()).unwrap();
            for target in attr_values(&xml, "Target") {
                let resolved = resolve_relationship_target(part, &target);
                assert!(
                    parts.contains_key(&resolved),
                    "{part} target {target} resolved to missing {resolved}"
                );
            }
        }
    }

    #[test]
    fn embeds_dm_sans_static_faces_without_subsetting() {
        let parts = zip_parts(&build_pptx(&sample()));
        let content_types = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
        let presentation = String::from_utf8(parts["ppt/presentation.xml"].clone()).unwrap();
        let rels = String::from_utf8(parts["ppt/_rels/presentation.xml.rels"].clone()).unwrap();

        assert!(content_types
            .contains("<Default Extension=\"fntdata\" ContentType=\"application/x-fontdata\"/>"));
        assert!(presentation.contains("embedTrueTypeFonts=\"1\""));
        assert!(presentation.contains("saveSubsetFonts=\"0\""));
        assert!(presentation
            .contains("<p:embeddedFontLst><p:embeddedFont><p:font typeface=\"DM Sans\"/>"));
        assert!(!presentation.contains("Trebuchet MS"));
        assert!(!presentation.contains("Calibri"));

        for (idx, font) in EMBEDDED_FONTS.iter().enumerate() {
            let rel_id = font_rel_id(6, idx);
            assert!(
                presentation.contains(&format!("<p:{} r:id=\"{}\"/>", font.role, rel_id)),
                "presentation must reference embedded {} face",
                font.role
            );
            assert!(
                rels.contains(&format!(
                    "Id=\"{}\" Type=\"{}/font\" Target=\"{}\"",
                    rel_id, R, font.target
                )),
                "presentation rels must target {}",
                font.target
            );
            let fontdata = parts
                .get(font.part)
                .unwrap_or_else(|| panic!("missing embedded font part {}", font.part));
            assert_eq!(read_u32(fontdata, 0) as usize, fontdata.len());
            assert_eq!(read_u32(fontdata, 4) as usize, font.ttf.len());
            assert_eq!(read_u32(fontdata, 8), 0x0002_0001);
            assert_eq!(read_u32(fontdata, 12), 0);
            assert!(
                fontdata.ends_with(font.ttf),
                "{} must embed the complete static TTF payload",
                font.part
            );
        }
    }

    #[test]
    fn trust_slide_uses_house_style_punctuation() {
        let parts = zip_parts(&build_pptx(&sample()));
        let slide = String::from_utf8(parts["ppt/slides/slide6.xml"].clone()).unwrap();
        assert!(slide.contains("<a:t>Verifiable by construction</a:t>"));
        assert!(slide.contains("No telemetry, license check, or upload path;"));
        assert!(!slide.contains("upload path — the audit records"));
    }

    #[test]
    fn title_tagline_uses_brand_colours() {
        let parts = zip_parts(&build_pptx(&sample()));
        let slide = String::from_utf8(parts["ppt/slides/slide1.xml"].clone()).unwrap();
        assert!(slide.contains(&geom(CONTENT_X, 3.15, CONTENT_W, 0.72)));
        assert!(slide.contains("<a:t>Global Data</a:t>"));
        assert!(slide.contains("<a:t> · </a:t>"));
        assert!(slide.contains("<a:t>Local Speeds</a:t>"));
        assert!(
            slide.contains(&format!(
                "<a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>Global Data</a:t>",
                TAGLINE_LIGHT, HEAD
            )),
            "Global Data should be light neutral on the dark title slide"
        );
        assert!(
            slide.contains(&format!(
                "<a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t> · </a:t>",
                TAGLINE_SEP, HEAD
            )),
            "tagline separator should use the muted brand tone"
        );
        assert!(
            slide.contains(&format!(
                "<a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>Local Speeds</a:t>",
                CYAN, HEAD
            )),
            "Local Speeds should be brand teal on the dark title slide"
        );
    }

    #[test]
    fn title_timestamp_uses_readable_utc_datetime() {
        assert_eq!(
            fmt_generated_at_display("2026-04-28T07:06:07+02:00"),
            "2026-04-28 05:06 UTC"
        );
        assert_eq!(fmt_generated_at_display("capture-run"), "capture-run");

        let parts = zip_parts(&build_pptx(&sample()));
        let slide = String::from_utf8(parts["ppt/slides/slide1.xml"].clone()).unwrap();
        assert!(slide.contains("<a:t>PostgreSQL 16.2"));
        assert!(slide.contains("generated 2026-04-28 00:00 UTC"));
        assert!(!slide.contains("2026-04-28T00:00:00Z"));
    }

    #[test]
    fn external_tables_are_listed_but_not_used_for_aggregate_scale() {
        let mut blueprint = sample();
        let mut external = table(
            99_000_000,
            99_000_000_000,
            9_000_000_000,
            false,
            &[("text", 1)],
            &[],
        );
        external.kind = "external".into();
        external.counted_in_totals = Some(false);
        blueprint.tables.insert("table-external".into(), external);

        let deck = analyze(&blueprint);
        assert_eq!(deck.tables.len(), 3, "external table remains inventoried");
        assert_eq!(counted_table_count(&deck), 2);
        assert!(deck
            .top_tables
            .iter()
            .all(|(table_id, _)| *table_id != "table-external"));
    }

    #[test]
    fn executive_summary_singularizes_counted_prose() {
        let parts = zip_parts(&build_pptx(&sample()));
        let slide2 = String::from_utf8(parts["ppt/slides/slide2.xml"].clone()).unwrap();

        assert!(slide2.contains("<a:t>2 tables, 13.5M rows, 4.2 GiB table data; 1 schema.</a:t>"));
        assert!(slide2.contains("<a:t>1 foreign-key link; connected tables: 2 of 2.</a:t>"));
        assert!(!slide2.contains("1 schemas"));
        assert!(!slide2.contains("1 foreign-key links"));

        let mut one = sample();
        let table_bytes = one.tables["table-001"].table_bytes;
        let index_bytes = one.tables["table-001"].index_bytes;
        one.tables.retain(|id, _| id == "table-001");
        one.fk_edges.clear();
        one.totals = Totals {
            table_count: 1,
            row_count: 1,
            table_bytes,
            index_bytes,
        };
        let one_parts = zip_parts(&build_pptx(&one));
        let one_title = String::from_utf8(one_parts["ppt/slides/slide1.xml"].clone()).unwrap();
        let one_exec = String::from_utf8(one_parts["ppt/slides/slide2.xml"].clone()).unwrap();

        assert!(one_title.contains("production   ·   1 table   ·   generated"));
        assert!(one_exec.contains("<a:t>1 table, 1 row, 3.9 GiB table data; 1 schema.</a:t>"));
        assert!(one_exec.contains(
            "<a:t>Largest table holds 100% of table data; plan the migration wave around it.</a:t>"
        ));
        assert!(!one_title.contains("1 tables"));
        assert!(!one_exec.contains("1 tables"));
        assert!(!one_exec.contains("Largest 1 tables"));
        assert!(!one_exec.contains("1 rows"));
    }

    #[test]
    fn deck_slides_avoid_raw_one_plus_plural_fragments() {
        let mut one = sample();
        let table_bytes = one.tables["table-001"].table_bytes;
        let index_bytes = one.tables["table-001"].index_bytes;
        one.tables.retain(|id, _| id == "table-001");
        one.fk_edges.clear();
        one.totals = Totals {
            table_count: 1,
            row_count: 1,
            table_bytes,
            index_bytes,
        };

        let blueprints = [
            one,
            parse_blueprint("saas-medium", include_str!("../samples/saas-medium.toml")),
            parse_blueprint(
                "ecommerce-large",
                include_str!("../samples/ecommerce-large.toml"),
            ),
        ];
        let bad_fragments = [
            "1 tables",
            "1 rows",
            "1 schemas",
            "1 foreign-key links",
            "Largest 1 tables",
            "1 more tables",
            "1 refs",
        ];

        for blueprint in blueprints {
            let parts = zip_parts(&build_pptx(&blueprint));
            for (name, bytes) in parts
                .iter()
                .filter(|(name, _)| name.starts_with("ppt/slides/slide"))
            {
                let xml = String::from_utf8(bytes.clone()).unwrap();
                for fragment in bad_fragments {
                    assert!(
                        !xml.contains(fragment),
                        "{name} contains ungrammatical count fragment {fragment:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn footer_matches_the_approved_house_geometry() {
        let parts = zip_parts(&build_pptx(&sample()));
        let title_slide = String::from_utf8(parts["ppt/slides/slide1.xml"].clone()).unwrap();
        let dark_slide = String::from_utf8(parts["ppt/slides/slide2.xml"].clone()).unwrap();
        let light_slide = String::from_utf8(parts["ppt/slides/slide3.xml"].clone()).unwrap();
        let dark_rels =
            String::from_utf8(parts["ppt/slides/_rels/slide2.xml.rels"].clone()).unwrap();
        let light_rels =
            String::from_utf8(parts["ppt/slides/_rels/slide3.xml.rels"].clone()).unwrap();

        assert!(dark_slide.contains(&format!(
            "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/>",
            emu(FOOTER_X),
            emu(FOOTER_LOGO_Y),
            emu(FOOTER_LOGO_W),
            emu(FOOTER_LOGO_H)
        )));
        assert!(dark_slide.contains(&format!(
            "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"0\"/>",
            emu(FOOTER_X),
            emu(FOOTER_RULE_Y),
            emu(FOOTER_RULE_W)
        )));
        assert!(dark_slide.contains(&format!(
            "<a:srgbClr val=\"{}\"/></a:solidFill><a:prstDash val=\"solid\"/>",
            FOOT_RULE_DK
        )));
        assert!(light_slide.contains(&format!(
            "<a:srgbClr val=\"{}\"/></a:solidFill><a:prstDash val=\"solid\"/>",
            LINE
        )));
        assert!(title_slide.contains("DBWarp logo for dark surfaces"));
        assert!(!title_slide.contains("DBWarp small logo"));
        assert!(dark_slide.contains("DBWarp small logo for dark surfaces"));
        assert!(!dark_slide.contains("DBWarp logo for dark surfaces"));
        assert!(dark_slide.contains(&format!("r:embed=\"{}\"", LOGO_DARK_SMALL_REL)));
        assert!(dark_rels.contains(&format!(
            "Id=\"{}\" Type=\"{}\" Target=\"../media/dbwarp-logo-dark-small.png\"",
            LOGO_DARK_SMALL_REL,
            format!("{}/image", R)
        )));
        assert!(light_slide.contains("DBWarp small logo for light surfaces"));
        assert!(!light_slide.contains("DBWarp logo for light surfaces"));
        assert!(light_slide.contains(&format!("r:embed=\"{}\"", LOGO_LIGHT_SMALL_REL)));
        assert!(light_rels.contains(&format!(
            "Id=\"{}\" Type=\"{}\" Target=\"../media/dbwarp-logo-light-small.png\"",
            LOGO_LIGHT_SMALL_REL,
            format!("{}/image", R)
        )));
        assert!(dark_slide.contains(&format!(
            "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/>",
            emu(FOOTER_PAGE_X),
            emu(FOOTER_TEXT_Y),
            emu(FOOTER_PAGE_W),
            emu(FOOTER_TEXT_H)
        )));
        assert!(dark_slide.contains(&format!(
            "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/>",
            emu(FOOTER_URL_X),
            emu(FOOTER_TEXT_Y),
            emu(FOOTER_URL_W),
            emu(FOOTER_TEXT_H)
        )));
        assert_eq!(
            emu(FOOTER_LOGO_Y) + emu(FOOTER_LOGO_H) / 2 + emu(0.01),
            emu(FOOTER_TEXT_Y) + emu(FOOTER_TEXT_H) / 2,
            "the logo is optically raised 0.01in against the text centre"
        );
        assert!(
            emu(FOOTER_PAGE_X) + emu(FOOTER_PAGE_W) / 2 == emu(13.33 / 2.0),
            "page number belongs on the approved 13.33in design midpoint"
        );
        assert!(
            emu(FOOTER_URL_X) > SLIDE_W / 2,
            "URL belongs in the right-hand footer region"
        );
        assert!(dark_slide.contains("<a:pPr algn=\"ctr\"/><a:r><a:rPr"));
        assert!(dark_slide.contains("<a:pPr algn=\"r\"/><a:r><a:rPr"));
        assert!(dark_slide.contains(&format!(
            "sz=\"1100\" b=\"0\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>2</a:t>",
            ICE, BODY_F
        )));
        assert!(dark_slide.contains(&format!(
            "sz=\"1100\" b=\"1\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>DBWarp.com</a:t>",
            CYAN_LT, BODY_F
        )));
        assert!(light_slide.contains(&format!(
            "sz=\"1100\" b=\"0\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>3</a:t>",
            MUTED, BODY_F
        )));
        assert!(light_slide.contains(&format!(
            "sz=\"1100\" b=\"1\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>DBWarp.com</a:t>",
            CYANDK, BODY_F
        )));
        assert!(light_slide.contains("<a:t>DBWarp.com</a:t>"));
        assert!(!title_slide.contains("<a:t>Confidential</a:t>"));
        assert!(!dark_slide.contains("<a:t>Confidential</a:t>"));
    }

    #[test]
    fn footer_confidentiality_is_optional_and_uses_the_reference_zone() {
        let parts = zip_parts(&build_pptx_with_confidentiality(
            &sample(),
            Some("Confidential"),
        ));
        let title_slide = String::from_utf8(parts["ppt/slides/slide1.xml"].clone()).unwrap();
        let dark_slide = String::from_utf8(parts["ppt/slides/slide2.xml"].clone()).unwrap();
        let light_slide = String::from_utf8(parts["ppt/slides/slide3.xml"].clone()).unwrap();
        let dot_geometry = format!(
            "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/>",
            emu(FOOTER_LOGO_INK_R),
            emu(FOOTER_TEXT_Y),
            emu(FOOTER_NOTE_X - FOOTER_LOGO_INK_R),
            emu(FOOTER_TEXT_H)
        );
        let note_geometry = format!(
            "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/>",
            emu(FOOTER_NOTE_X),
            emu(FOOTER_TEXT_Y),
            emu(FOOTER_NOTE_W),
            emu(FOOTER_TEXT_H)
        );

        assert!(!title_slide.contains("<a:t>Confidential</a:t>"));
        for slide in [&dark_slide, &light_slide] {
            assert!(slide.contains(&dot_geometry));
            assert!(slide.contains(&note_geometry));
            assert!(slide.contains("<a:t>·</a:t>"));
            assert!(slide.contains("<a:t>Confidential</a:t>"));
        }
        assert!(dark_slide.contains(&format!(
            "<a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>Confidential</a:t>",
            FOOT_DK, BODY_F
        )));
        assert!(light_slide.contains(&format!(
            "<a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>Confidential</a:t>",
            MUTED, BODY_F
        )));
    }

    #[test]
    fn major_blocks_share_common_content_bounds() {
        let parts = zip_parts(&build_pptx(&sample()));
        let slide2 = String::from_utf8(parts["ppt/slides/slide2.xml"].clone()).unwrap();
        let slide3 = String::from_utf8(parts["ppt/slides/slide3.xml"].clone()).unwrap();
        let final_slide = String::from_utf8(parts["ppt/slides/slide6.xml"].clone()).unwrap();

        let card_w = 5.55;
        let card_gap = 0.4;
        let right_card_x = CONTENT_X + card_w + card_gap;
        assert_eq!(emu(right_card_x + card_w), emu(CONTENT_R));
        assert!(slide2.contains(&geom(CONTENT_X, 2.1, card_w, 1.45)));
        assert!(slide2.contains(&geom(right_card_x, 2.1, card_w, 1.45)));
        assert!(slide2.contains(&geom(CONTENT_X, 4.05, card_w, 1.45)));
        assert!(slide2.contains(&geom(right_card_x, 4.05, card_w, 1.45)));

        assert!(slide3.contains(&geom(
            CONTENT_X,
            OVERVIEW_SCHEMA_ROW_Y,
            CONTENT_W,
            OVERVIEW_SCHEMA_ROW_H
        )));
        assert!(slide3.contains(&geom(1.18, 1.69, 2.0, 0.22)));
        assert!(slide3.contains(&geom(
            OVERVIEW_SCHEMA_COUNT_X,
            OVERVIEW_SCHEMA_COUNT_Y,
            OVERVIEW_SCHEMA_VALUE_W,
            0.38
        )));
        let count_geom = geom(
            OVERVIEW_SCHEMA_COUNT_X,
            OVERVIEW_SCHEMA_COUNT_Y,
            OVERVIEW_SCHEMA_VALUE_W,
            0.38,
        );
        let count_pos = slide3.find(&count_geom).expect("schema count geometry");
        let count_window = &slide3[count_pos..slide3.len().min(count_pos + 2200)];
        assert!(
            count_window.contains("<a:pPr algn=\"l\"/>"),
            "schema count should align to the schema label's left edge"
        );
        assert!(count_window.contains(&format!(
            "sz=\"2050\" b=\"1\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>1</a:t>",
            INK, HEAD
        )));
        assert!(count_window.contains(&format!(
            "sz=\"1320\" b=\"0\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t> independent namespace included in this Blueprint</a:t>",
            BODY, BODY_F
        )));
        assert!(slide3.contains("<a:t>SCHEMA</a:t>"));
        assert!(slide3.contains("<a:t> independent namespace included in this Blueprint</a:t>"));
        assert!(!slide3.contains("included in the blueprint"));
        assert!(slide3.contains(&geom(CONTENT_X, OVERVIEW_PRIMARY_ROW_Y, CONTENT_W, 1.2)));
        let panel_gap = 0.2;
        let panel_w = (CONTENT_W - panel_gap) / 2.0;
        let right_panel_x = CONTENT_X + panel_w + panel_gap;
        assert_eq!(emu(right_panel_x + panel_w), emu(CONTENT_R));
        assert!(slide3.contains(&geom(CONTENT_X, 3.83, panel_w, 1.8)));
        assert!(slide3.contains(&geom(right_panel_x, 3.83, panel_w, 1.8)));
        assert!(slide3.contains(&geom(CONTENT_X + 0.28, OVERVIEW_METRIC_NOTE_Y, 2.12, 0.4)));
        assert!(slide3.contains(&geom(
            right_panel_x + 0.3,
            OVERVIEW_METRIC_NOTE_Y,
            1.45,
            0.4
        )));
        assert!(slide3.contains(&geom(
            right_panel_x + 1.95,
            OVERVIEW_METRIC_NOTE_Y,
            1.45,
            0.4
        )));
        assert!(slide3.contains(&geom(right_panel_x + 3.6, OVERVIEW_METRIC_NOTE_Y, 2.0, 0.4)));
        assert!(slide3.contains(&geom(CONTENT_X, 5.82, CONTENT_W, 0.5)));
        assert!(slide3.contains("<a:t>PRIMARY SIZING INPUTS</a:t>"));
        assert!(slide3.contains("<a:t>Columns</a:t>"));
        assert!(!slide3.contains("<a:t>columns</a:t>"));
        assert!(slide3.contains("<a:t>Avg cols/table</a:t>"));
        assert!(slide3.contains("<a:t>Secondary structure</a:t>"));
        assert!(slide3.contains("<a:t>index storage size</a:t>"));
        assert!(slide3.contains(&format!(
            "sz=\"1900\" b=\"1\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>{}</a:t>",
            INK,
            HEAD,
            fmt_bytes(sample().totals.index_bytes)
        )));

        let ethos_gap = 0.27;
        let ethos_w = (CONTENT_W - 2.0 * ethos_gap) / 3.0;
        let third_ethos_x = CONTENT_X + 2.0 * (ethos_w + ethos_gap);
        assert_eq!(emu(third_ethos_x + ethos_w), emu(CONTENT_R));
        assert!(final_slide.contains(&geom(third_ethos_x, 2.35, ethos_w, 2.65)));
        assert!(final_slide.contains(&geom(CONTENT_X, 5.4, CONTENT_W, 0.4)));
    }

    #[test]
    fn overview_schema_row_pluralizes_namespace_copy() {
        let single_parts = zip_parts(&build_pptx(&sample()));
        let single_slide =
            String::from_utf8(single_parts["ppt/slides/slide3.xml"].clone()).unwrap();
        assert!(single_slide.contains("<a:t>SCHEMA</a:t>"));
        assert!(
            single_slide.contains("<a:t> independent namespace included in this Blueprint</a:t>")
        );
        assert!(
            !single_slide.contains("<a:t> independent namespaces included in this Blueprint</a:t>")
        );

        let mut multi = sample();
        multi
            .tables
            .get_mut("table-002")
            .expect("sample table must exist")
            .schema = "schema-B".to_string();
        let multi_parts = zip_parts(&build_pptx(&multi));
        let multi_slide = String::from_utf8(multi_parts["ppt/slides/slide3.xml"].clone()).unwrap();
        assert!(multi_slide.contains("<a:t>SCHEMAS</a:t>"));
        assert!(
            multi_slide.contains("<a:t> independent namespaces included in this Blueprint</a:t>")
        );
        assert!(
            !multi_slide.contains("<a:t> independent namespace included in this Blueprint</a:t>")
        );
    }

    #[test]
    fn overview_schema_fact_flows_in_one_text_box_for_large_counts() {
        let blueprint = sample();
        let mut deck = analyze(&blueprint);
        deck.schemas = 12_345;

        let slide = build_overview(&deck).render();
        let fact_geom = geom(
            OVERVIEW_SCHEMA_COUNT_X,
            OVERVIEW_SCHEMA_COUNT_Y,
            OVERVIEW_SCHEMA_VALUE_W,
            0.38,
        );
        let fact_pos = slide.find(&fact_geom).expect("schema fact geometry");
        let fact_window = &slide[fact_pos..slide.len().min(fact_pos + 2400)];

        let count_pos = fact_window
            .find("<a:t>12345</a:t>")
            .expect("large schema count");
        let copy_pos = fact_window
            .find("<a:t> independent namespaces included in this Blueprint</a:t>")
            .expect("namespace copy follows schema count");
        assert!(
            count_pos < copy_pos,
            "schema count and namespace copy should flow in order within one text box"
        );
        assert!(fact_window.contains("sz=\"2050\""));
        assert!(fact_window.contains("sz=\"1320\""));
        assert!(
            !fact_window.contains("<a:t> independent namespace included in this Blueprint</a:t>")
        );
    }

    #[test]
    fn overview_metric_labels_pluralize_visible_counts() {
        let mut plural = sample();
        plural
            .fk_edges
            .get_mut("table-002")
            .expect("sample edge list must exist")
            .push(FkEdge {
                to: "table-001".into(),
                cols: vec![3],
                to_cols: vec![1],
                ..FkEdge::default()
            });
        let plural_parts = zip_parts(&build_pptx(&plural));
        let plural_slide = String::from_utf8(plural_parts["ppt/slides/slide3.xml"].clone())
            .expect("slide 3 should be utf-8");

        assert!(plural_slide.contains("<a:t>Tables</a:t>"));
        assert!(plural_slide.contains("<a:t>Foreign keys</a:t>"));
        assert!(plural_slide.contains("<a:t>Indexes</a:t>"));

        let mut one = sample();
        let table_bytes = one.tables["table-001"].table_bytes;
        let index_bytes = one.tables["table-001"].index_bytes;
        one.tables.retain(|id, _| id == "table-001");
        one.tables
            .get_mut("table-001")
            .expect("sample table must exist")
            .idxs
            .retain(|id, _| id == "idx-1");
        one.fk_edges.clear();
        one.fk_edges.insert(
            "table-001".to_string(),
            vec![FkEdge {
                to: "table-001".into(),
                cols: vec![1],
                to_cols: vec![1],
                ..FkEdge::default()
            }],
        );
        one.totals = Totals {
            table_count: 1,
            row_count: 1,
            table_bytes,
            index_bytes,
        };
        let one_parts = zip_parts(&build_pptx(&one));
        let one_slide = String::from_utf8(one_parts["ppt/slides/slide3.xml"].clone())
            .expect("slide 3 should be utf-8");

        assert!(one_slide.contains("<a:t>Table</a:t>"));
        assert!(one_slide.contains("<a:t>Foreign key</a:t>"));
        assert!(one_slide.contains("<a:t>Index</a:t>"));
        assert!(!one_slide.contains("<a:t>Tables</a:t>"));
        assert!(!one_slide.contains("<a:t>Foreign keys</a:t>"));
        assert!(!one_slide.contains("<a:t>Indexes</a:t>"));
    }

    #[test]
    fn small_schema_map_slide_uses_light_theme() {
        let parts = zip_parts(&build_pptx(&sample()));
        let slide5 = String::from_utf8(parts["ppt/slides/slide5.xml"].clone()).unwrap();
        let light_bg = format!(
            "<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            WHITE
        );
        let dark_bg = format!(
            "<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            INK
        );

        assert!(slide5.contains(&light_bg));
        assert!(!slide5.contains(&dark_bg));
        assert!(slide5.contains("DBWarp small logo for light surfaces"));
        assert!(slide5.contains(&geom(CONTENT_X, 3.1, 4.3, 1.95)));
        assert!(slide5.contains(&geom(CONTENT_R - 4.3, 3.1, 4.3, 1.95)));
        assert!(slide5.contains(&format!(
            "sz=\"1100\" b=\"1\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>DBWarp.com</a:t>",
            CYANDK, BODY_F
        )));
    }

    #[test]
    fn aggregate_foreign_key_relationship_slide_uses_light_theme() {
        let blueprint = parse_blueprint("saas-medium", include_str!("../samples/saas-medium.toml"));
        let parts = zip_parts(&build_pptx(&blueprint));
        let slide2 = String::from_utf8(parts["ppt/slides/slide2.xml"].clone()).unwrap();
        let relationship_slide = String::from_utf8(parts["ppt/slides/slide6.xml"].clone()).unwrap();
        let light_bg = format!(
            "<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            WHITE
        );
        let dark_bg = format!(
            "<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            INK
        );

        assert!(slide2.contains(&dark_bg));
        assert!(relationship_slide.contains(&light_bg));
        assert!(!slide2.contains(&light_bg));
        assert!(!relationship_slide.contains(&dark_bg));
        assert!(relationship_slide.contains(&format!("<a:t>{}</a:t>", tr("deck.relationships"))));
        assert!(relationship_slide.contains(&format!(
            "<a:t>{}</a:t>",
            tr("deck.foreign_key_relationships")
        )));
        assert!(relationship_slide.contains("DBWarp small logo for light surfaces"));
        assert!(slide2.contains(&format!(
            "sz=\"1100\" b=\"0\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>2</a:t>",
            ICE, BODY_F
        )));
        assert!(slide2.contains(&format!(
            "sz=\"1100\" b=\"1\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>DBWarp.com</a:t>",
            CYAN_LT, BODY_F
        )));
        assert!(relationship_slide.contains(&format!(
            "sz=\"1100\" b=\"0\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>6</a:t>",
            MUTED, BODY_F
        )));
        assert!(relationship_slide.contains(&format!(
            "sz=\"1100\" b=\"1\" i=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>DBWarp.com</a:t>",
            CYANDK, BODY_F
        )));
    }

    #[test]
    fn checked_in_blueprint_cases_generate_valid_decks() {
        let write_dir =
            std::env::var_os("DBWARP_BLUEPRINT_WRITE_SAMPLE_DECKS").map(std::path::PathBuf::from);
        if let Some(dir) = &write_dir {
            std::fs::create_dir_all(dir).unwrap();
        }

        let mut cases = checked_in_blueprint_cases();
        let mut tier2 = parse_blueprint(
            "pg-tier2-synthetic",
            include_str!("../tests/fixtures/blueprint_format/pg_expected.toml"),
        );
        add_compression(&mut tier2, "table-001", 3.5, false);
        add_compression(&mut tier2, "table-002", 2.0, true);
        cases.push(("pg-tier2-synthetic", tier2));

        for (name, blueprint) in cases {
            let deck = build_pptx(&blueprint);
            let parts = zip_parts(&deck);
            assert!(
                parts.contains_key("ppt/presentation.xml"),
                "{name} must include presentation.xml"
            );
            assert!(
                parts.keys().any(|p| p.starts_with("ppt/slides/slide")),
                "{name} must include slides"
            );
            if name.contains("tier2") {
                let xml = String::from_utf8_lossy(&deck);
                assert!(
                    xml.contains("Measured compression (Tier 2)"),
                    "{name} should include compression slide"
                );
            }
            if let Some(dir) = &write_dir {
                std::fs::write(dir.join(format!("{name}.pptx")), &deck).unwrap();
            }
        }
    }

    #[test]
    fn deterministic() {
        assert_eq!(build_pptx(&sample()), build_pptx(&sample()));
        assert_eq!(
            build_pptx_with_confidentiality(&sample(), Some("Confidential")),
            build_pptx_with_confidentiality(&sample(), Some("Confidential"))
        );
        assert_ne!(
            build_pptx(&sample()),
            build_pptx_with_confidentiality(&sample(), Some("Confidential"))
        );
    }

    #[test]
    fn bytes_format_keeps_sub_mib_values_readable() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(16 * 1024), "16 KiB");
        assert_eq!(fmt_bytes(512 * 1024), "512 KiB");
        assert_eq!(fmt_bytes(2 * 1_048_576), "2.0 MiB");
    }

    #[test]
    fn four_table_detail_layout_stays_on_slide() {
        let mut sf = sample();
        sf.tables.insert(
            "table-003".to_string(),
            table(
                25_000,
                524_288,
                65_536,
                false,
                &[("integer", 1), ("text", 2)],
                &[("btree", false, vec![1])],
            ),
        );
        sf.tables.insert(
            "table-004".to_string(),
            table(
                5_000,
                131_072,
                16_384,
                false,
                &[("uuid", 1), ("timestamp", 2)],
                &[("btree", true, vec![1])],
            ),
        );
        let d = analyze(&sf);
        let xml = build_tables(&d).render();
        assert!(
            max_bottom_emu(&xml) <= SLIDE_H,
            "four-table detail slide exceeded slide height"
        );
    }

    #[test]
    fn tier2_compression_slide_is_rendered() {
        let mut sf = sample();
        add_compression(&mut sf, "table-001", 3.5, false);
        add_compression(&mut sf, "table-002", 2.0, true);
        let b = build_pptx(&sf);
        let s = String::from_utf8_lossy(&b);
        assert!(s.contains("Measured compression (Tier 2)"));
        assert!(s.contains("Weighted zstd-3"));
        assert!(s.contains("Projected compressed"));
        assert!(
            s.contains("ppt/slides/slide7.xml"),
            "compression adds one slide to small schema deck with FK"
        );
        assert!(!s.contains("ppt/slides/slide8.xml"));
    }

    #[test]
    fn large_schema_uses_characterization_slides() {
        let mut sf = sample();
        for i in 3..=40u32 {
            sf.tables.insert(
                format!("table-{:03}", i),
                table(
                    1000 * i as u64,
                    1_000_000 * i as u64,
                    100_000,
                    true,
                    &[("integer", 1), ("text", 2)],
                    &[("btree", true, vec![1])],
                ),
            );
        }
        sf.totals.table_count = sf.tables.len() as u64;
        let b = build_pptx(&sf);
        let s = String::from_utf8_lossy(&b);
        assert!(
            s.contains("ppt/slides/slide7.xml"),
            "large schema => 7 slides"
        );
        assert!(!s.contains("ppt/slides/slide8.xml"));
    }

    #[test]
    fn table_size_slides_use_brand_bar_colours() {
        let small_parts = zip_parts(&build_pptx(&sample()));
        let small_table_slide = String::from_utf8(small_parts["ppt/slides/slide4.xml"].clone())
            .expect("slide4 should be valid UTF-8 XML");
        assert!(small_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", TABLE_SIZE_TRACK)));
        assert!(small_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", TABLE_SIZE_BAR)));
        assert!(!small_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", PGLT)));
        assert!(!small_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", PG)));

        let large = parse_blueprint("saas-medium", include_str!("../samples/saas-medium.toml"));
        let large_parts = zip_parts(&build_pptx(&large));
        let largest_table_slide = String::from_utf8(large_parts["ppt/slides/slide4.xml"].clone())
            .expect("slide4 should be valid UTF-8 XML");
        assert!(largest_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", TABLE_SIZE_TRACK)));
        assert!(largest_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", TABLE_SIZE_BAR)));
        assert!(!largest_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", PGLT)));
        assert!(!largest_table_slide.contains(&format!("<a:srgbClr val=\"{}\"/>", PG)));
    }

    fn max_bottom_emu(xml: &str) -> i64 {
        let mut max = 0;
        let mut rest = xml;
        while let Some(off_idx) = rest.find("<a:off ") {
            let after_off = &rest[off_idx..];
            let Some(y) = attr_i64(after_off, "y") else {
                break;
            };
            let Some(ext_idx) = after_off.find("<a:ext ") else {
                break;
            };
            let after_ext = &after_off[ext_idx..];
            let Some(cy) = attr_i64(after_ext, "cy") else {
                break;
            };
            max = max.max(y + cy);
            rest = &after_ext[1..];
        }
        max
    }

    fn geom(x: f64, y: f64, w: f64, h: f64) -> String {
        format!(
            "<a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/>",
            emu(x),
            emu(y),
            emu(w),
            emu(h)
        )
    }

    fn attr_i64(s: &str, name: &str) -> Option<i64> {
        let needle = format!("{name}=\"");
        let start = s.find(&needle)? + needle.len();
        let end = s[start..].find('"')? + start;
        s[start..end].parse().ok()
    }

    fn zip_parts(zip: &[u8]) -> BTreeMap<String, Vec<u8>> {
        let eocd = zip
            .windows(4)
            .rposition(|w| w == [0x50, 0x4b, 0x05, 0x06])
            .expect("EOCD");
        let count = read_u16(zip, eocd + 10) as usize;
        let cd_size = read_u32(zip, eocd + 12) as usize;
        let cd_off = read_u32(zip, eocd + 16) as usize;
        assert_eq!(
            eocd,
            cd_off + cd_size,
            "central directory must end immediately before EOCD"
        );

        let mut parts = BTreeMap::new();
        let mut off = cd_off;
        for _ in 0..count {
            assert_eq!(
                read_u32(zip, off),
                0x0201_4b50,
                "central directory signature"
            );
            assert_eq!(
                read_u16(zip, off + 10),
                0,
                "deck zip must use stored entries"
            );
            let crc = read_u32(zip, off + 16);
            let comp_size = read_u32(zip, off + 20) as usize;
            let size = read_u32(zip, off + 24) as usize;
            let nlen = read_u16(zip, off + 28) as usize;
            let xlen = read_u16(zip, off + 30) as usize;
            let clen = read_u16(zip, off + 32) as usize;
            let local_off = read_u32(zip, off + 42) as usize;
            let name = std::str::from_utf8(&zip[off + 46..off + 46 + nlen])
                .unwrap()
                .to_string();
            off += 46 + nlen + xlen + clen;

            assert_eq!(comp_size, size, "stored size for {name}");
            assert_eq!(
                read_u32(zip, local_off),
                0x0403_4b50,
                "local header signature for {name}"
            );
            assert_eq!(
                read_u16(zip, local_off + 8),
                0,
                "local header method for {name}"
            );
            let local_nlen = read_u16(zip, local_off + 26) as usize;
            let local_xlen = read_u16(zip, local_off + 28) as usize;
            let local_name =
                std::str::from_utf8(&zip[local_off + 30..local_off + 30 + local_nlen]).unwrap();
            assert_eq!(local_name, name, "local and central names must match");
            let data_start = local_off + 30 + local_nlen + local_xlen;
            let data_end = data_start + size;
            let data = zip[data_start..data_end].to_vec();
            assert_eq!(crc32(&data), crc, "CRC mismatch for {name}");
            parts.insert(name, data);
        }
        assert_eq!(off, cd_off + cd_size, "central directory size");
        assert_eq!(parts.len(), count, "central directory count");
        parts
    }

    fn read_u16(data: &[u8], off: usize) -> u16 {
        u16::from_le_bytes(data[off..off + 2].try_into().unwrap())
    }

    fn read_u32(data: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(data[off..off + 4].try_into().unwrap())
    }

    fn attr_values(xml: &str, attr: &str) -> Vec<String> {
        let needle = format!("{attr}=\"");
        let mut out = Vec::new();
        let mut rest = xml;
        while let Some(idx) = rest.find(&needle) {
            let val_start = idx + needle.len();
            let Some(val_end) = rest[val_start..].find('"') else {
                break;
            };
            out.push(rest[val_start..val_start + val_end].to_string());
            rest = &rest[val_start + val_end + 1..];
        }
        out
    }

    fn resolve_relationship_target(part: &str, target: &str) -> String {
        let base = if part == "_rels/.rels" {
            ""
        } else {
            part.find("/_rels/").map(|idx| &part[..idx]).unwrap_or("")
        };
        let joined = if target.starts_with('/') || base.is_empty() {
            target.trim_start_matches('/').to_string()
        } else {
            format!("{base}/{target}")
        };
        let mut parts = Vec::new();
        for p in joined.split('/') {
            match p {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                _ => parts.push(p),
            }
        }
        parts.join("/")
    }
}

//! Offline PowerPoint (.pptx) deck generator for a `BlueprintFile`.
//!
//! Authors the OOXML directly — no third-party crate, no network, and no
//! runtime asset lookup. The current DBWarp lockups and static DM Sans faces
//! are embedded as package parts so PowerPoint renders the approved brand
//! without requiring a local font install. Deterministic: the same `BlueprintFile` + pinned
//! `generated_at` produces byte-identical output. Adaptive: management summary
//! first, then detailed per-table view for small schemas or characterization
//! slides (largest tables, type composition, relationship hubs) for large ones.
//! Closes with a "verifiable by construction" trust-model slide.

use std::collections::BTreeMap;

use crate::format::{BlueprintFile, BlueprintTable, Totals};
use crate::i18n::Locale;

fn tr(key: &str) -> &'static str {
    crate::i18n::text(key)
}

fn trf(key: &str, values: &[(&str, String)]) -> String {
    crate::i18n::format(key, values)
}

// --- namespaces -------------------------------------------------------------
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const PR: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const XMLDECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n";

// --- brand assets and palette ----------------------------------------------
const LOGO_DARK_PNG: &[u8] = include_bytes!("../assets/brand/dbwarp-logo-dark.png");
const LOGO_LIGHT_PNG: &[u8] = include_bytes!("../assets/brand/dbwarp-logo-light.png");
const LOGO_DARK_SMALL_PNG: &[u8] = include_bytes!("../assets/brand/dbwarp-logo-dark-small.png");
const LOGO_LIGHT_SMALL_PNG: &[u8] = include_bytes!("../assets/brand/dbwarp-logo-light-small.png");
const LOGO_DARK_REL: &str = "rId2";
const LOGO_LIGHT_REL: &str = "rId3";
const LOGO_DARK_SMALL_REL: &str = "rId4";
const LOGO_LIGHT_SMALL_REL: &str = "rId5";
const LOGO_ASPECT: f64 = 268.0 / 64.0;
const LOGO_SMALL_MAX_H: f64 = 0.5;

struct EmbeddedFontFace {
    role: &'static str,
    part: &'static str,
    target: &'static str,
    ttf: &'static [u8],
}

const DM_SANS_REGULAR_TTF: &[u8] = include_bytes!("../assets/fonts/dm-sans/DMSans-Regular.ttf");
const DM_SANS_ITALIC_TTF: &[u8] = include_bytes!("../assets/fonts/dm-sans/DMSans-Italic.ttf");
const DM_SANS_BOLD_TTF: &[u8] = include_bytes!("../assets/fonts/dm-sans/DMSans-Bold.ttf");
const DM_SANS_BOLD_ITALIC_TTF: &[u8] =
    include_bytes!("../assets/fonts/dm-sans/DMSans-BoldItalic.ttf");
const EMBEDDED_FONTS: &[EmbeddedFontFace] = &[
    EmbeddedFontFace {
        role: "regular",
        part: "ppt/fonts/DMSans-Regular.fntdata",
        target: "fonts/DMSans-Regular.fntdata",
        ttf: DM_SANS_REGULAR_TTF,
    },
    EmbeddedFontFace {
        role: "bold",
        part: "ppt/fonts/DMSans-Bold.fntdata",
        target: "fonts/DMSans-Bold.fntdata",
        ttf: DM_SANS_BOLD_TTF,
    },
    EmbeddedFontFace {
        role: "italic",
        part: "ppt/fonts/DMSans-Italic.fntdata",
        target: "fonts/DMSans-Italic.fntdata",
        ttf: DM_SANS_ITALIC_TTF,
    },
    EmbeddedFontFace {
        role: "boldItalic",
        part: "ppt/fonts/DMSans-BoldItalic.fntdata",
        target: "fonts/DMSans-BoldItalic.fntdata",
        ttf: DM_SANS_BOLD_ITALIC_TTF,
    },
];

const INK: &str = "0B1220";
const INK2: &str = "111A2C";
const WHITE: &str = "FFFFFF";
const PAPER: &str = "F6F8FC";
const LINE: &str = "E2E8F0";
const CYAN: &str = "2DD4BF";
const CYANDK: &str = "0F766E";
const AQUA: &str = "5EEAD4";
const ICE: &str = "C7D3E2";
const TAGLINE_LIGHT: &str = "E7ECF3";
const TAGLINE_SEP: &str = "9FB2C7";
const MUTED: &str = "64748B";
const FOOT_DK: &str = "8AA0B8";
const FOOT_RULE_DK: &str = "2E3E58";
const BODY: &str = "1E293B";
const PG: &str = "2F6491";
const PGLT: &str = "E7EEF5";
const GREEN_BG: &str = "ECFDF6";
const GREEN_LN: &str = "A7F3D0";
const CYAN_LT: &str = "99F6E4";
const TABLE_SIZE_BAR: &str = CYANDK;
const TABLE_SIZE_TRACK: &str = GREEN_BG;
const DM_SANS: &str = "DM Sans";
const HEAD: &str = DM_SANS;
const BODY_F: &str = DM_SANS;

const SLIDE_W: i64 = 12_192_000;
const SLIDE_H: i64 = 6_858_000;
const CONTENT_X: f64 = 0.9;
const CONTENT_R: f64 = 12.4;
const CONTENT_W: f64 = CONTENT_R - CONTENT_X;
const OVERVIEW_SCHEMA_ROW_Y: f64 = 1.5;
const OVERVIEW_SCHEMA_ROW_H: f64 = 0.85;
const OVERVIEW_PRIMARY_ROW_Y: f64 = 2.47;
const OVERVIEW_SCHEMA_COUNT_X: f64 = 1.18;
const OVERVIEW_SCHEMA_COUNT_Y: f64 = 1.94;
const OVERVIEW_SCHEMA_VALUE_W: f64 = CONTENT_R - OVERVIEW_SCHEMA_COUNT_X - 0.76;
const OVERVIEW_METRIC_VALUE_Y: f64 = 4.33;
const OVERVIEW_METRIC_LABEL_Y: f64 = 4.66;
const OVERVIEW_METRIC_NOTE_Y: f64 = 4.93;

// Footer geometry is measured from the approved DBWarp mentor-deck reference.
// The content slides use a five-part footer: rule, small logo, optional
// classification, bare centred page number, and right-aligned DBWarp.com.
const FOOTER_X: f64 = 0.7;
const FOOTER_RULE_Y: f64 = 6.883;
const FOOTER_RULE_W: f64 = 11.93;
const FOOTER_LOGO_Y: f64 = 6.92;
const FOOTER_LOGO_W: f64 = 0.84;
const FOOTER_LOGO_H: f64 = 0.20;
const FOOTER_TEXT_Y: f64 = 6.90;
const FOOTER_TEXT_H: f64 = 0.26;
const FOOTER_LOGO_INK_R: f64 = 1.462;
const FOOTER_NOTE_X: f64 = 1.771;
const FOOTER_NOTE_W: f64 = 4.20;
const FOOTER_PAGE_X: f64 = 6.415;
const FOOTER_PAGE_W: f64 = 0.50;
const FOOTER_URL_X: f64 = 10.58;
const FOOTER_URL_W: f64 = 2.05;

fn emu(inch: f64) -> i64 {
    (inch * 914_400.0).round() as i64
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn engine_name(e: &str) -> String {
    match e {
        "postgresql" => "PostgreSQL".to_string(),
        "mysql" => "MySQL".to_string(),
        "sqlserver" => "SQL Server".to_string(),
        other => other.to_string(),
    }
}

fn commafy(n: u64) -> String {
    let s = n.to_string();
    let b = s.as_bytes();
    let len = b.len();
    let mut out = String::new();
    for (idx, &c) in b.iter().enumerate() {
        if idx > 0 && (len - idx) % 3 == 0 {
            out.push(',');
        }
        out.push(c as char);
    }
    out
}

fn fmt_rows(n: u64) -> String {
    if n >= 1_000_000 {
        let v = n as f64 / 1_000_000.0;
        if v >= 100.0 {
            format!("{}M", v.round() as i64)
        } else {
            let s = format!("{:.1}", v);
            let s = s.trim_end_matches('0').trim_end_matches('.');
            format!("{}M", s)
        }
    } else if n >= 1000 {
        format!("{}K", (n as f64 / 1000.0).round() as i64)
    } else {
        n.to_string()
    }
}

fn fmt_bytes(b: u64) -> String {
    if b >= 1_073_741_824 {
        let g = b as f64 / 1_073_741_824.0;
        if g >= 10.0 {
            format!("{} GiB", g.round() as i64)
        } else {
            format!("{:.1} GiB", g)
        }
    } else if b >= 1_048_576 {
        let m = b as f64 / 1_048_576.0;
        if m >= 10.0 {
            format!("{} MiB", m.round() as i64)
        } else {
            format!("{:.1} MiB", m)
        }
    } else if b >= 1024 {
        format!("{} KiB", (b as f64 / 1024.0).round() as i64)
    } else {
        format!("{} B", b)
    }
}

fn fmt_ratio(v: f64) -> String {
    if v.is_finite() && v > 0.0 {
        format!("{:.2}x", v)
    } else {
        "n/a".to_string()
    }
}

fn fmt_pct(v: f64) -> String {
    if v.is_finite() {
        format!("{:.0}%", v)
    } else {
        "n/a".to_string()
    }
}

fn fmt_share_pct(v: f64) -> String {
    if !v.is_finite() || v <= 0.0 {
        "0%".to_string()
    } else if v < 0.005 {
        "<1%".to_string()
    } else {
        format!("{:.0}%", v * 100.0)
    }
}

fn fmt_avg_per_table(total: u64, tables: u64) -> String {
    if tables == 0 {
        "0.0".to_string()
    } else {
        format!("{:.1}", total as f64 / tables as f64)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CountCategory {
    One,
    Few,
    Other,
}

fn count_category(count: u64) -> CountCategory {
    if count == 1 {
        CountCategory::One
    } else if crate::i18n::active_locale() == Locale::Pl
        && matches!(count % 10, 2..=4)
        && !matches!(count % 100, 12..=14)
    {
        CountCategory::Few
    } else {
        CountCategory::Other
    }
}

fn count_phrase(
    count: u64,
    display: String,
    one_key: &'static str,
    few_key: &'static str,
    other_key: &'static str,
) -> String {
    let key = match count_category(count) {
        CountCategory::One => one_key,
        CountCategory::Few => few_key,
        CountCategory::Other => other_key,
    };
    trf(key, &[("count", display)])
}

fn count_label(
    count: u64,
    one_key: &'static str,
    few_key: &'static str,
    other_key: &'static str,
) -> &'static str {
    match count_category(count) {
        CountCategory::One => tr(one_key),
        CountCategory::Few => tr(few_key),
        CountCategory::Other => tr(other_key),
    }
}

fn table_count_phrase(count: u64) -> String {
    count_phrase(
        count,
        commafy(count),
        "deck.count.table.one",
        "deck.count.table.few",
        "deck.count.table.other",
    )
}

fn table_metric_label(count: u64) -> &'static str {
    count_label(
        count,
        "deck.metric.table.one",
        "deck.metric.table.few",
        "deck.metric.table.other",
    )
}

fn row_count_phrase(count: u64) -> String {
    count_phrase(
        count,
        fmt_rows(count),
        "deck.count.row.one",
        "deck.count.row.few",
        "deck.count.row.other",
    )
}

fn schema_count_phrase(count: usize) -> String {
    count_phrase(
        count as u64,
        count.to_string(),
        "deck.count.schema.one",
        "deck.count.schema.few",
        "deck.count.schema.other",
    )
}

fn foreign_key_metric_label(count: usize) -> &'static str {
    count_label(
        count as u64,
        "deck.metric.foreign_key.one",
        "deck.metric.foreign_key.few",
        "deck.metric.foreign_key.other",
    )
}

fn foreign_key_link_count_phrase(count: usize) -> String {
    count_phrase(
        count as u64,
        commafy(count as u64),
        "deck.count.foreign_key_link.one",
        "deck.count.foreign_key_link.few",
        "deck.count.foreign_key_link.other",
    )
}

fn index_metric_label(count: u64) -> &'static str {
    count_label(
        count,
        "deck.metric.index.one",
        "deck.metric.index.few",
        "deck.metric.index.other",
    )
}

fn schema_label_key(schemas: usize) -> &'static str {
    if schemas == 1 {
        "deck.schema"
    } else {
        "deck.schemas"
    }
}

fn schema_namespace_key(schemas: usize) -> &'static str {
    if schemas == 1 {
        "deck.schema_namespace"
    } else {
        "deck.schema_namespaces"
    }
}

fn fmt_generated_at_display(generated: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(generated)
        .map(|dt| {
            dt.with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
        })
        .unwrap_or_else(|_| generated.to_string())
}

include!("deck_layout.rs");
include!("deck_analysis.rs");
include!("deck_slides.rs");
include!("deck_ooxml.rs");
include!("deck_fonts.rs");
include!("deck_zip.rs");
/// Build a complete `.pptx` (OOXML, stored-zip) for the given Blueprint. No I/O,
/// no network: returns the bytes for the caller to write.
#[cfg(test)]
pub fn build_pptx(blueprint: &BlueprintFile) -> Vec<u8> {
    build_pptx_with_confidentiality(blueprint, None)
}

pub(crate) fn build_pptx_with_confidentiality(
    blueprint: &BlueprintFile,
    confidentiality: Option<&str>,
) -> Vec<u8> {
    let d = analyze(blueprint);
    let mut slides: Vec<SlideB> = Vec::new();
    slides.push(build_title(&d));
    slides.push(build_executive(&d));
    slides.push(build_overview(&d));
    if d.tables.len() <= 4 {
        slides.push(build_tables(&d));
    } else {
        slides.push(build_largest(&d));
        slides.push(build_composition(&d));
    }
    if let Some(slide) = build_compression(&d) {
        slides.push(slide);
    }
    if d.edges_count > 0 && d.edges_count <= 3 {
        if let Some(fk) = d.fk {
            slides.push(build_schema(&d, fk));
        }
    } else if d.edges_count > 0 {
        slides.push(build_relationships(&d));
    }
    slides.push(build_ethos(&d));

    for (idx, sl) in slides.iter_mut().enumerate() {
        if idx > 0 {
            let dark = sl.bg == Some(INK);
            sl.footer((idx + 1) as u32, dark, confidentiality);
        }
    }
    let n = slides.len();

    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    parts.push(("[Content_Types].xml".into(), content_types(n).into_bytes()));
    parts.push(("_rels/.rels".into(), root_rels().into_bytes()));
    parts.push((
        "docProps/core.xml".into(),
        core_xml(d.generated).into_bytes(),
    ));
    parts.push(("docProps/app.xml".into(), app_xml().into_bytes()));
    parts.push((
        "ppt/presentation.xml".into(),
        presentation_xml(n).into_bytes(),
    ));
    parts.push((
        "ppt/_rels/presentation.xml.rels".into(),
        presentation_rels(n).into_bytes(),
    ));
    parts.push(("ppt/presProps.xml".into(), presprops().into_bytes()));
    parts.push(("ppt/viewProps.xml".into(), viewprops().into_bytes()));
    parts.push(("ppt/tableStyles.xml".into(), tablestyles().into_bytes()));
    parts.push(("ppt/theme/theme1.xml".into(), theme().into_bytes()));
    parts.push((
        "ppt/media/dbwarp-logo-dark.png".into(),
        LOGO_DARK_PNG.to_vec(),
    ));
    parts.push((
        "ppt/media/dbwarp-logo-light.png".into(),
        LOGO_LIGHT_PNG.to_vec(),
    ));
    parts.push((
        "ppt/media/dbwarp-logo-dark-small.png".into(),
        LOGO_DARK_SMALL_PNG.to_vec(),
    ));
    parts.push((
        "ppt/media/dbwarp-logo-light-small.png".into(),
        LOGO_LIGHT_SMALL_PNG.to_vec(),
    ));
    for font in EMBEDDED_FONTS {
        parts.push((font.part.into(), eot_font_data(font.ttf)));
    }
    parts.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        slide_master().into_bytes(),
    ));
    parts.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        slide_master_rels().into_bytes(),
    ));
    parts.push((
        "ppt/slideLayouts/slideLayout1.xml".into(),
        slide_layout().into_bytes(),
    ));
    parts.push((
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        slide_layout_rels().into_bytes(),
    ));
    for (k, sl) in slides.iter().enumerate() {
        let n1 = k + 1;
        parts.push((
            format!("ppt/slides/slide{}.xml", n1),
            sl.render().into_bytes(),
        ));
        parts.push((
            format!("ppt/slides/_rels/slide{}.xml.rels", n1),
            slide_rels().into_bytes(),
        ));
    }
    parts.sort_by(|a, b| a.0.cmp(&b.0));
    write_zip(&parts)
}

#[cfg(test)]
include!("deck_tests.rs");

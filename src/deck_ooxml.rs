// --- OOXML package scaffolding ---------------------------------------------
fn part_rels(items: &[(&str, String, &str)]) -> String {
    let mut s = String::from(XMLDECL);
    s.push_str(&format!("<Relationships xmlns=\"{}\">", PR));
    for (id, ty, tgt) in items {
        s.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>",
            id, ty, tgt
        ));
    }
    s.push_str("</Relationships>");
    s
}
fn content_types(n_slides: usize) -> String {
    let mut ov: Vec<(String, &str)> = vec![
        (
            "/ppt/presentation.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        ),
        (
            "/ppt/slideMasters/slideMaster1.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml",
        ),
        (
            "/ppt/slideLayouts/slideLayout1.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml",
        ),
        (
            "/ppt/theme/theme1.xml".into(),
            "application/vnd.openxmlformats-officedocument.theme+xml",
        ),
        (
            "/ppt/presProps.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml",
        ),
        (
            "/ppt/viewProps.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml",
        ),
        (
            "/ppt/tableStyles.xml".into(),
            "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml",
        ),
        (
            "/docProps/core.xml".into(),
            "application/vnd.openxmlformats-package.core-properties+xml",
        ),
        (
            "/docProps/app.xml".into(),
            "application/vnd.openxmlformats-officedocument.extended-properties+xml",
        ),
    ];
    for k in 1..=n_slides {
        ov.push((
            format!("/ppt/slides/slide{}.xml", k),
            "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
        ));
    }
    let mut s = String::from(XMLDECL);
    s.push_str(&format!("<Types xmlns=\"{}\">", CT));
    s.push_str("<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>");
    s.push_str("<Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    s.push_str("<Default Extension=\"png\" ContentType=\"image/png\"/>");
    s.push_str("<Default Extension=\"fntdata\" ContentType=\"application/x-fontdata\"/>");
    for (pn, c) in &ov {
        s.push_str(&format!(
            "<Override PartName=\"{}\" ContentType=\"{}\"/>",
            pn, c
        ));
    }
    s.push_str("</Types>");
    s
}

fn font_rel_id(n_slides: usize, font_idx: usize) -> String {
    format!("rId{}", 2 + n_slides + 4 + font_idx)
}

fn embedded_font_list(n_slides: usize) -> String {
    let mut faces = String::new();
    for (idx, font) in EMBEDDED_FONTS.iter().enumerate() {
        faces.push_str(&format!(
            "<p:{} r:id=\"{}\"/>",
            font.role,
            font_rel_id(n_slides, idx)
        ));
    }
    format!(
        "<p:embeddedFontLst><p:embeddedFont><p:font typeface=\"{}\"/>{}</p:embeddedFont></p:embeddedFontLst>",
        DM_SANS, faces
    )
}

fn presentation_xml(n_slides: usize) -> String {
    let mut sld = String::new();
    for k in 0..n_slides {
        sld.push_str(&format!(
            "<p:sldId id=\"{}\" r:id=\"rId{}\"/>",
            256 + k,
            2 + k
        ));
    }
    let embedded_fonts = embedded_font_list(n_slides);
    format!(
        "{decl}<p:presentation xmlns:a=\"{A}\" xmlns:r=\"{R}\" xmlns:p=\"{P}\" embedTrueTypeFonts=\"1\" saveSubsetFonts=\"0\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst>{sld}</p:sldIdLst><p:sldSz cx=\"{w}\" cy=\"{h}\" type=\"screen16x9\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/>{embedded_fonts}</p:presentation>",
        decl = XMLDECL,
        A = A,
        R = R,
        P = P,
        sld = sld,
        w = SLIDE_W,
        h = SLIDE_H,
        embedded_fonts = embedded_fonts
    )
}

fn presentation_rels(n_slides: usize) -> String {
    let mut items: Vec<(&str, String, &str)> = Vec::new();
    let mut owned_ids: Vec<String> = Vec::new();
    let mut owned_targets: Vec<String> = Vec::new();
    owned_ids.push("rId1".into());
    owned_targets.push("slideMasters/slideMaster1.xml".into());
    for k in 0..n_slides {
        owned_ids.push(format!("rId{}", 2 + k));
        owned_targets.push(format!("slides/slide{}.xml", k + 1));
    }
    let base = 2 + n_slides;
    let extra = [
        ("presProps", "presProps.xml"),
        ("viewProps", "viewProps.xml"),
        ("theme", "theme/theme1.xml"),
        ("tableStyles", "tableStyles.xml"),
    ];
    for (off, (rel, tgt)) in extra.iter().enumerate() {
        owned_ids.push(format!("rId{}", base + off));
        owned_targets.push((*tgt).to_string());
        let _ = rel;
    }
    for (idx, font) in EMBEDDED_FONTS.iter().enumerate() {
        owned_ids.push(font_rel_id(n_slides, idx));
        owned_targets.push(font.target.to_string());
    }
    // build items with proper types
    items.push((
        owned_ids[0].as_str(),
        format!("{}/slideMaster", R),
        owned_targets[0].as_str(),
    ));
    for k in 0..n_slides {
        items.push((
            owned_ids[1 + k].as_str(),
            format!("{}/slide", R),
            owned_targets[1 + k].as_str(),
        ));
    }
    let exoff = 1 + n_slides;
    let extypes = [
        format!("{}/presProps", R),
        format!("{}/viewProps", R),
        format!("{}/theme", R),
        format!("{}/tableStyles", R),
    ];
    for j in 0..4 {
        items.push((
            owned_ids[exoff + j].as_str(),
            extypes[j].clone(),
            owned_targets[exoff + j].as_str(),
        ));
    }
    let fontoff = exoff + 4;
    for j in 0..EMBEDDED_FONTS.len() {
        items.push((
            owned_ids[fontoff + j].as_str(),
            format!("{}/font", R),
            owned_targets[fontoff + j].as_str(),
        ));
    }
    part_rels(&items)
}

fn root_rels() -> String {
    part_rels(&[
        (
            "rId1",
            format!("{}/officeDocument", R),
            "ppt/presentation.xml",
        ),
        (
            "rId2",
            format!("{}/metadata/core-properties", PR),
            "docProps/core.xml",
        ),
        (
            "rId3",
            format!("{}/extended-properties", R),
            "docProps/app.xml",
        ),
    ])
}

fn slide_rels() -> String {
    part_rels(&[
        (
            "rId1",
            format!("{}/slideLayout", R),
            "../slideLayouts/slideLayout1.xml",
        ),
        (
            LOGO_DARK_REL,
            format!("{}/image", R),
            "../media/dbwarp-logo-dark.png",
        ),
        (
            LOGO_LIGHT_REL,
            format!("{}/image", R),
            "../media/dbwarp-logo-light.png",
        ),
        (
            LOGO_DARK_SMALL_REL,
            format!("{}/image", R),
            "../media/dbwarp-logo-dark-small.png",
        ),
        (
            LOGO_LIGHT_SMALL_REL,
            format!("{}/image", R),
            "../media/dbwarp-logo-light-small.png",
        ),
    ])
}

fn slide_master() -> String {
    format!(
        "{decl}<p:sldMaster xmlns:a=\"{A}\" xmlns:r=\"{R}\" xmlns:p=\"{P}\"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{white}\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/><p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst></p:sldMaster>",
        decl = XMLDECL, A = A, R = R, P = P, white = WHITE
    )
}

fn slide_master_rels() -> String {
    part_rels(&[
        (
            "rId1",
            format!("{}/slideLayout", R),
            "../slideLayouts/slideLayout1.xml",
        ),
        ("rId2", format!("{}/theme", R), "../theme/theme1.xml"),
    ])
}

fn slide_layout() -> String {
    format!(
        "{decl}<p:sldLayout xmlns:a=\"{A}\" xmlns:r=\"{R}\" xmlns:p=\"{P}\" type=\"blank\" preserve=\"1\"><p:cSld name=\"Blank\"><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>",
        decl = XMLDECL, A = A, R = R, P = P
    )
}

fn slide_layout_rels() -> String {
    part_rels(&[(
        "rId1",
        format!("{}/slideMaster", R),
        "../slideMasters/slideMaster1.xml",
    )])
}

fn clr(tag: &str, hexv: &str) -> String {
    format!(
        "<a:{tag}><a:srgbClr val=\"{hexv}\"/></a:{tag}>",
        tag = tag,
        hexv = hexv
    )
}

fn theme() -> String {
    let mut s = String::from(XMLDECL);
    s.push_str(&format!(
        "<a:theme xmlns:a=\"{}\" name=\"dbwarp\"><a:themeElements><a:clrScheme name=\"dbwarp\">",
        A
    ));
    s.push_str(&clr("dk1", INK));
    s.push_str(&clr("lt1", WHITE));
    s.push_str(&clr("dk2", INK2));
    s.push_str(&clr("lt2", PAPER));
    s.push_str(&clr("accent1", CYAN));
    s.push_str(&clr("accent2", PG));
    s.push_str(&clr("accent3", CYANDK));
    s.push_str(&clr("accent4", MUTED));
    s.push_str(&clr("accent5", ICE));
    s.push_str(&clr("accent6", GREEN_LN));
    s.push_str(&clr("hlink", CYAN));
    s.push_str(&clr("folHlink", CYANDK));
    s.push_str("</a:clrScheme><a:fontScheme name=\"dbwarp\">");
    s.push_str(&format!("<a:majorFont><a:latin typeface=\"{}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>", HEAD));
    s.push_str(&format!("<a:minorFont><a:latin typeface=\"{}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont>", BODY_F));
    s.push_str("</a:fontScheme><a:fmtScheme name=\"dbwarp\"><a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:fillStyleLst><a:lnStyleLst><a:ln w=\"6350\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln><a:ln w=\"12700\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln><a:ln w=\"19050\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>");
    s
}

fn presprops() -> String {
    format!(
        "{}<p:presentationPr xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\"/>",
        XMLDECL, A, R, P
    )
}

fn viewprops() -> String {
    format!(
        "{}<p:viewPr xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\"/>",
        XMLDECL, A, R, P
    )
}

fn tablestyles() -> String {
    format!(
        "{}<a:tblStyleLst xmlns:a=\"{}\" def=\"{{5940675A-B579-460E-94D1-54222C63F5DA}}\"/>",
        XMLDECL, A
    )
}

fn core_xml(generated: &str) -> String {
    format!(
        "{decl}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>dbwarp \u{2014} {title}</dc:title><dc:creator>dbwarp-blueprint</dc:creator><cp:lastModifiedBy>dbwarp-blueprint</cp:lastModifiedBy><dcterms:created xsi:type=\"dcterms:W3CDTF\">{g}</dcterms:created><dcterms:modified xsi:type=\"dcterms:W3CDTF\">{g}</dcterms:modified></cp:coreProperties>",
        decl = XMLDECL,
        title = esc(tr("deck.report")),
        g = esc(generated)
    )
}

fn app_xml() -> String {
    format!("{}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><Application>dbwarp-blueprint</Application></Properties>", XMLDECL)
}

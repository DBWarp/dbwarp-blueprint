// --- text model ------------------------------------------------------------
struct Run {
    t: String,
    sz: i64,
    b: bool,
    i: bool,
    color: &'static str,
    face: &'static str,
    spc: i64,
}
fn run(
    t: impl Into<String>,
    sz: i64,
    b: bool,
    i: bool,
    color: &'static str,
    face: &'static str,
) -> Run {
    Run {
        t: t.into(),
        sz,
        b,
        i,
        color,
        face,
        spc: 0,
    }
}

struct Para {
    align: &'static str,
    space_before: i64,
    runs: Vec<Run>,
}

fn para(align: &'static str, runs: Vec<Run>) -> Para {
    Para {
        align,
        space_before: 0,
        runs,
    }
}

// --- slide builder ---------------------------------------------------------
struct SlideB {
    shapes: Vec<String>,
    bg: Option<&'static str>,
    id: u32,
}

impl SlideB {
    fn new(bg: Option<&'static str>) -> Self {
        SlideB {
            shapes: Vec::new(),
            bg,
            id: 1,
        }
    }

    fn nid(&mut self) -> u32 {
        self.id += 1;
        self.id
    }

    fn rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: Option<&str>,
        line: Option<&str>,
        line_pt: f64,
        round: Option<i64>,
    ) {
        let i = self.nid();
        let geom = match round {
            Some(adj) => format!(
                "<a:prstGeom prst=\"roundRect\"><a:avLst><a:gd name=\"adj\" fmla=\"val {}\"/></a:avLst></a:prstGeom>",
                adj
            ),
            None => "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>".to_string(),
        };
        let fillx = match fill {
            Some(c) => format!("<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>", c),
            None => "<a:noFill/>".to_string(),
        };
        let lnx = match line {
            Some(c) => format!(
                "<a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:ln>",
                (line_pt * 12_700.0) as i64,
                c
            ),
            None => String::new(),
        };
        self.shapes.push(format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{i}\" name=\"r{i}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm>{geom}{fillx}{lnx}</p:spPr></p:sp>",
            i = i, x = emu(x), y = emu(y), w = emu(w), h = emu(h), geom = geom, fillx = fillx, lnx = lnx
        ));
    }

    fn line(&mut self, x: f64, y: f64, w: f64, color: &str, line_pt: f64) {
        let i = self.nid();
        self.shapes.push(format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{i}\" name=\"l{i}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"0\"/></a:xfrm><a:prstGeom prst=\"line\"><a:avLst/></a:prstGeom><a:noFill/><a:ln w=\"{line_w}\"><a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln></p:spPr></p:sp>",
            i = i,
            x = emu(x),
            y = emu(y),
            w = emu(w),
            line_w = (line_pt * 12_700.0) as i64,
            color = color,
        ));
    }

    fn arrow(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str) {
        let i = self.nid();
        self.shapes.push(format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{i}\" name=\"a{i}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rightArrow\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"{fill}\"/></a:solidFill></p:spPr></p:sp>",
            i = i, x = emu(x), y = emu(y), w = emu(w), h = emu(h), fill = fill
        ));
    }

    fn text(&mut self, x: f64, y: f64, w: f64, h: f64, paras: Vec<Para>, anchor: &str) {
        let i = self.nid();
        let mut body = String::new();
        for pa in &paras {
            let ppr = if pa.space_before > 0 {
                format!(
                    "<a:pPr algn=\"{}\"><a:spcBef><a:spcPts val=\"{}\"/></a:spcBef></a:pPr>",
                    pa.align,
                    pa.space_before * 100
                )
            } else {
                format!("<a:pPr algn=\"{}\"/>", pa.align)
            };
            let mut runs = String::new();
            for rn in &pa.runs {
                let spc = if rn.spc != 0 {
                    format!(" spc=\"{}\"", rn.spc)
                } else {
                    String::new()
                };
                runs.push_str(&format!(
                    "<a:r><a:rPr lang=\"{}\" sz=\"{}\" b=\"{}\" i=\"{}\"{}><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>{}</a:t></a:r>",
                    crate::i18n::active_locale().bcp47(),
                    rn.sz,
                    if rn.b { 1 } else { 0 },
                    if rn.i { 1 } else { 0 },
                    spc,
                    rn.color,
                    rn.face,
                    esc(&rn.t)
                ));
            }
            body.push_str(&format!("<a:p>{}{}</a:p>", ppr, runs));
        }
        self.shapes.push(format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{i}\" name=\"t{i}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap=\"square\" lIns=\"0\" tIns=\"0\" rIns=\"0\" bIns=\"0\" anchor=\"{anchor}\"><a:noAutofit/></a:bodyPr><a:lstStyle/>{body}</p:txBody></p:sp>",
            i = i, x = emu(x), y = emu(y), w = emu(w), h = emu(h), anchor = anchor, body = body
        ));
    }

    fn image(&mut self, x: f64, y: f64, w: f64, h: f64, rel_id: &str, name: &str) {
        let i = self.nid();
        self.shapes.push(format!(
            "<p:pic><p:nvPicPr><p:cNvPr id=\"{i}\" name=\"{name}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"{rel_id}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",
            i = i,
            name = esc(name),
            rel_id = rel_id,
            x = emu(x),
            y = emu(y),
            w = emu(w),
            h = emu(h),
        ));
    }

    fn brand_logo(&mut self, x: f64, y: f64, h: f64, dark: bool) {
        let small = h < LOGO_SMALL_MAX_H;
        let (rel_id, name) = match (dark, small) {
            (true, true) => (LOGO_DARK_SMALL_REL, "DBWarp small logo for dark surfaces"),
            (false, true) => (LOGO_LIGHT_SMALL_REL, "DBWarp small logo for light surfaces"),
            (true, false) => (LOGO_DARK_REL, "DBWarp logo for dark surfaces"),
            (false, false) => (LOGO_LIGHT_REL, "DBWarp logo for light surfaces"),
        };
        self.image(x, y, h * LOGO_ASPECT, h, rel_id, name);
    }

    fn footer(&mut self, page: u32, dark: bool, confidentiality: Option<&str>) {
        let text_col = if dark { ICE } else { MUTED };
        let note_col = if dark { FOOT_DK } else { MUTED };
        let url_col = if dark { CYAN_LT } else { CYANDK };
        let (logo_rel, logo_name) = if dark {
            (LOGO_DARK_SMALL_REL, "DBWarp small logo for dark surfaces")
        } else {
            (LOGO_LIGHT_SMALL_REL, "DBWarp small logo for light surfaces")
        };

        self.line(
            FOOTER_X,
            FOOTER_RULE_Y,
            FOOTER_RULE_W,
            if dark { FOOT_RULE_DK } else { LINE },
            1.0,
        );
        self.image(
            FOOTER_X,
            FOOTER_LOGO_Y,
            FOOTER_LOGO_W,
            FOOTER_LOGO_H,
            logo_rel,
            logo_name,
        );
        if let Some(label) = confidentiality.filter(|value| !value.is_empty()) {
            self.text(
                FOOTER_LOGO_INK_R,
                FOOTER_TEXT_Y,
                FOOTER_NOTE_X - FOOTER_LOGO_INK_R,
                FOOTER_TEXT_H,
                vec![para(
                    "ctr",
                    vec![run("·", 1100, false, false, note_col, BODY_F)],
                )],
                "ctr",
            );
            self.text(
                FOOTER_NOTE_X,
                FOOTER_TEXT_Y,
                FOOTER_NOTE_W,
                FOOTER_TEXT_H,
                vec![para(
                    "l",
                    vec![run(label, 1100, false, false, note_col, BODY_F)],
                )],
                "ctr",
            );
        }
        self.text(
            FOOTER_PAGE_X,
            FOOTER_TEXT_Y,
            FOOTER_PAGE_W,
            FOOTER_TEXT_H,
            vec![para(
                "ctr",
                vec![run(page.to_string(), 1100, false, false, text_col, BODY_F)],
            )],
            "ctr",
        );
        self.text(
            FOOTER_URL_X,
            FOOTER_TEXT_Y,
            FOOTER_URL_W,
            FOOTER_TEXT_H,
            vec![para(
                "r",
                vec![run(tr("deck.website"), 1100, true, false, url_col, BODY_F)],
            )],
            "ctr",
        );
    }

    fn render(&self) -> String {
        let bg = match self.bg {
            Some(c) => format!(
                "<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>",
                c
            ),
            None => String::new(),
        };
        format!(
            "{decl}<p:sld xmlns:a=\"{A}\" xmlns:r=\"{R}\" xmlns:p=\"{P}\"><p:cSld>{bg}<p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
            decl = XMLDECL, A = A, R = R, P = P, bg = bg, shapes = self.shapes.concat()
        )
    }
}

fn kicker_title(s: &mut SlideB, kicker: &str, title: &str, dark: bool) {
    let kcol = if dark { CYAN } else { CYANDK };
    s.text(
        0.9,
        0.62,
        9.0,
        0.3,
        vec![Para {
            align: "l",
            space_before: 0,
            runs: vec![Run {
                t: kicker.to_string(),
                sz: 1200,
                b: true,
                i: false,
                color: kcol,
                face: HEAD,
                spc: 200,
            }],
        }],
        "t",
    );
    let tcol = if dark { WHITE } else { INK };
    s.text(
        0.9,
        0.95,
        11.5,
        0.7,
        vec![para("l", vec![run(title, 3000, true, false, tcol, HEAD)])],
        "t",
    );
}

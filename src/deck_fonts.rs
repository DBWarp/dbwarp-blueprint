// PowerPoint stores embedded TrueType/OpenType faces as EOT fontdata parts.
// The font payload remains the full static TTF; the EOT flags stay zero so no
// subsetting, compression, or XOR transformation is declared.
fn eot_font_data(font: &[u8]) -> Vec<u8> {
    let os2 = sfnt_table(font, b"OS/2").expect("embedded DM Sans font must include OS/2");
    let head = sfnt_table(font, b"head").expect("embedded DM Sans font must include head");
    let panose = os2
        .get(32..42)
        .expect("embedded DM Sans OS/2 table must include PANOSE");
    let weight = be_u16(os2, 4).expect("embedded DM Sans OS/2 table must include weight") as u32;
    let fs_type = be_u16(os2, 8).expect("embedded DM Sans OS/2 table must include fsType");
    let fs_selection = be_u16(os2, 62).unwrap_or(0);
    let italic = (fs_selection & 0x0001) as u8;
    let unicode_ranges = [
        be_u32(os2, 42).expect("embedded DM Sans OS/2 table must include UnicodeRange1"),
        be_u32(os2, 46).expect("embedded DM Sans OS/2 table must include UnicodeRange2"),
        be_u32(os2, 50).expect("embedded DM Sans OS/2 table must include UnicodeRange3"),
        be_u32(os2, 54).expect("embedded DM Sans OS/2 table must include UnicodeRange4"),
    ];
    let codepage_ranges = [be_u32(os2, 82).unwrap_or(0), be_u32(os2, 86).unwrap_or(0)];
    let checksum =
        be_u32(head, 8).expect("embedded DM Sans head table must include checksum adjustment");

    let family = sfnt_name(font, 1).unwrap_or_else(|| DM_SANS.to_string());
    let style = sfnt_name(font, 2).unwrap_or_else(|| {
        if italic == 1 {
            "Italic".to_string()
        } else {
            "Regular".to_string()
        }
    });
    let version = sfnt_name(font, 5).unwrap_or_default();
    let full_name = sfnt_name(font, 4).unwrap_or_else(|| format!("{family} {style}"));

    let mut variable = Vec::new();
    push_eot_name(&mut variable, &family);
    push_eot_name(&mut variable, &style);
    push_eot_name(&mut variable, &version);
    push_eot_name(&mut variable, &full_name);
    push_le_u16(&mut variable, 0); // RootStringSize: empty root string permits document embedding.

    let eot_size = 82 + variable.len() + font.len();
    let mut out = Vec::with_capacity(eot_size);
    push_le_u32(&mut out, eot_size as u32);
    push_le_u32(&mut out, font.len() as u32);
    push_le_u32(&mut out, 0x0002_0001);
    push_le_u32(&mut out, 0);
    out.extend_from_slice(panose);
    out.push(0x01);
    out.push(italic);
    push_le_u32(&mut out, weight);
    push_le_u16(&mut out, fs_type);
    push_le_u16(&mut out, 0x504c);
    for range in unicode_ranges {
        push_le_u32(&mut out, range);
    }
    for range in codepage_ranges {
        push_le_u32(&mut out, range);
    }
    push_le_u32(&mut out, checksum);
    for _ in 0..4 {
        push_le_u32(&mut out, 0);
    }
    push_le_u16(&mut out, 0);
    debug_assert_eq!(out.len(), 82);
    out.extend_from_slice(&variable);
    out.extend_from_slice(font);
    debug_assert_eq!(out.len(), eot_size);
    out
}
fn sfnt_table<'a>(font: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let num_tables = be_u16(font, 4)? as usize;
    let table_dir_end = 12usize.checked_add(num_tables.checked_mul(16)?)?;
    if table_dir_end > font.len() {
        return None;
    }
    for idx in 0..num_tables {
        let record = 12 + idx * 16;
        if &font[record..record + 4] != tag {
            continue;
        }
        let offset = be_u32(font, record + 8)? as usize;
        let len = be_u32(font, record + 12)? as usize;
        let end = offset.checked_add(len)?;
        return font.get(offset..end);
    }
    None
}

fn sfnt_name(font: &[u8], name_id: u16) -> Option<String> {
    let table = sfnt_table(font, b"name")?;
    let count = be_u16(table, 2)? as usize;
    let string_offset = be_u16(table, 4)? as usize;
    let records_end = 6usize.checked_add(count.checked_mul(12)?)?;
    if records_end > table.len() || string_offset > table.len() {
        return None;
    }

    let mut best: Option<(u8, String)> = None;
    for idx in 0..count {
        let record = 6 + idx * 12;
        let platform = be_u16(table, record)?;
        let encoding = be_u16(table, record + 2)?;
        let language = be_u16(table, record + 4)?;
        let this_name_id = be_u16(table, record + 6)?;
        let len = be_u16(table, record + 8)? as usize;
        let offset = be_u16(table, record + 10)? as usize;
        if this_name_id != name_id {
            continue;
        }
        let score = if platform == 3 && language == 0x0409 && (encoding == 1 || encoding == 10) {
            0
        } else if platform == 3 && language == 0x0409 {
            1
        } else if platform == 3 {
            2
        } else if platform == 0 {
            3
        } else {
            continue;
        };
        let start = string_offset.checked_add(offset)?;
        let end = start.checked_add(len)?;
        let decoded = decode_utf16be(table.get(start..end)?)?;
        if best
            .as_ref()
            .map_or(true, |(best_score, _)| score < *best_score)
        {
            best = Some((score, decoded));
        }
    }
    best.map(|(_, name)| name)
}

fn decode_utf16be(bytes: &[u8]) -> Option<String> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

fn push_eot_name(out: &mut Vec<u8>, value: &str) {
    let mut bytes = Vec::new();
    for unit in value.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    assert!(
        bytes.len() <= u16::MAX as usize,
        "embedded DM Sans name record is too long"
    );
    push_le_u16(out, bytes.len() as u16);
    out.extend_from_slice(&bytes);
    push_le_u16(out, 0);
}

fn be_u16(data: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(off..off + 2)?.try_into().ok()?))
}

fn be_u32(data: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(off..off + 4)?.try_into().ok()?))
}

fn push_le_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_le_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

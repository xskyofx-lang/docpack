//! The colors an Excel workbook marks cells with: a cell's fill, the color of
//! its text, and pieces of a rich-text cell in a color of their own. calamine
//! reads values only, so these come from the workbook's own parts:
//! `styles.xml` (each cell style's fill and font), the theme (colors written
//! as "accent 2, 40 % lighter"), `sharedStrings.xml` (rich text) and each
//! sheet's `<c s="…">`.
use crate::color;
use crate::ooxml::{self, attr, local, Zip};
use quick_xml::events::Event;
use std::collections::HashMap;

/// Excel's default indexed palette (`indexed="n"`), used when the workbook
/// does not write its own (`<indexedColors>`).
const INDEXED: [&str; 64] = [
    "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF",
    "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF",
    "800000", "008000", "000080", "808000", "800080", "008080", "C0C0C0", "808080",
    "9999FF", "993366", "FFFFCC", "CCFFFF", "660066", "FF8080", "0066CC", "CCCCFF",
    "000080", "FF00FF", "FFFF00", "00FFFF", "800080", "800000", "008080", "0000FF",
    "00CCFF", "CCFFFF", "CCFFCC", "FFFF99", "99CCFF", "FF99CC", "CC99FF", "FFCC99",
    "3366FF", "33CCCC", "99CC00", "FFCC00", "FF9900", "FF6600", "666699", "969696",
    "003366", "339966", "003300", "333300", "993300", "993366", "333399", "333333",
];

/// What a cell shows in color: its fill, the color of all its text, and the
/// pieces of a rich-text cell (text, color).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Cell { pub fill: Option<String>, pub color: Option<String>, pub pieces: Vec<(String, Option<String>)> }

/// What the workbook says about colors, read once for all its sheets.
#[derive(Default)]
pub struct Book {
    /// Per cell style (`cellXfs`, the `s` of a cell): fill and font color.
    styles: Vec<(Option<String>, Option<String>)>,
    /// Rich shared strings: their pieces, only when a piece has a color.
    rich: HashMap<usize, Vec<(String, Option<String>)>>,
    /// Sheet name → its part (`xl/worksheets/sheet1.xml`).
    sheets: HashMap<String, String>,
}

/// One color as the workbook writes it: `rgb="FFRRGGBB"`, `theme="4"` with a
/// `tint`, or `indexed="10"`; `auto` and unknown values are no color.
fn resolve(e: &quick_xml::events::BytesStart, theme: &[String], indexed: &[String]) -> Option<String> {
    if attr(e, "auto").as_deref() == Some("1") { return None }
    let base = if let Some(argb) = attr(e, "rgb") {
        let h = argb.trim();
        color::normal(if h.len() == 8 { &h[2..] } else { h })
    } else if let Some(t) = attr(e, "theme").and_then(|v| v.parse::<usize>().ok()) {
        // Excel numbers the first four theme colors light-dark, the theme
        // file writes them dark-light (dk1, lt1, dk2, lt2).
        let i = match t { 0 => 1, 1 => 0, 2 => 3, 3 => 2, i => i };
        theme.get(i).cloned()
    } else if let Some(i) = attr(e, "indexed").and_then(|v| v.parse::<usize>().ok()) {
        indexed.get(i).cloned()
    } else { None }?;
    match attr(e, "tint").and_then(|v| v.parse::<f64>().ok()) {
        Some(t) if t != 0.0 => color::tint(&base, t),
        _ => Some(base),
    }
}

fn theme_colors(zip: &mut Zip) -> Vec<String> {
    let Ok(Some(xml)) = ooxml::part(zip, "xl/theme/theme1.xml") else { return Vec::new() };
    let mut out = Vec::new();
    let mut in_scheme = false;
    let mut reader = quick_xml::Reader::from_str(&xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == "clrScheme" => in_scheme = true,
            Ok(Event::End(e)) if local(e.name().as_ref()) == "clrScheme" => break,
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if in_scheme => {
                let name = e.name().as_ref().to_string();
                match local(&name) {
                    "srgbClr" => if let Some(v) = attr(&e, "val").and_then(|v| color::normal(&v)) { out.push(v) },
                    "sysClr" => if let Some(v) = attr(&e, "lastClr").and_then(|v| color::normal(&v)) { out.push(v) },
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Reads styles, theme, rich text and the sheets' parts. None for a workbook
/// that is not Office Open XML (`.xls`, `.ods`, `.xlsb`): no colors are read.
pub fn read(bytes: &[u8]) -> Option<Book> {
    let mut zip = ooxml::open(bytes).ok()?;
    let theme = theme_colors(&mut zip);
    let styles_xml = ooxml::part(&mut zip, "xl/styles.xml").ok()??;
    let mut indexed: Vec<String> = INDEXED.iter().map(|s| s.to_string()).collect();
    let (mut fonts, mut fills, mut xfs) = (Vec::<Option<String>>::new(), Vec::<Option<String>>::new(), Vec::<(usize, usize)>::new());
    {
        // First pass: a workbook's own palette replaces the default one.
        let mut reader = quick_xml::Reader::from_str(&styles_xml);
        let mut custom = Vec::new();
        loop {
            match reader.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if local(e.name().as_ref()) == "rgbColor" => {
                    if let Some(v) = attr(&e, "rgb").and_then(|v| color::normal(if v.len() == 8 { &v[2..] } else { &v })) { custom.push(v) }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        if !custom.is_empty() { indexed = custom }
    }
    let mut reader = quick_xml::Reader::from_str(&styles_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut solid = false;
    loop {
        let event = reader.read_event();
        match &event {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local(e.name().as_ref()).to_string();
                let inside = |s: &str| stack.iter().any(|x| x == s);
                match name.as_str() {
                    "font" if inside("fonts") => fonts.push(None),
                    "color" if inside("font") && inside("fonts") => { if let Some(last) = fonts.last_mut() { *last = resolve(e, &theme, &indexed) } }
                    "fill" if inside("fills") => { fills.push(None); solid = false }
                    "patternFill" if inside("fills") => solid = attr(e, "patternType").as_deref() == Some("solid"),
                    "fgColor" if inside("fills") && solid => { if let Some(last) = fills.last_mut() { *last = resolve(e, &theme, &indexed) } }
                    "xf" if inside("cellXfs") => xfs.push((
                        attr(e, "fontId").and_then(|v| v.parse().ok()).unwrap_or(0),
                        attr(e, "fillId").and_then(|v| v.parse().ok()).unwrap_or(0),
                    )),
                    _ => {}
                }
                if matches!(event, Ok(Event::Start(_))) { stack.push(name) }
            }
            Ok(Event::End(_)) => { stack.pop(); }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let body = fonts.first().cloned().flatten().unwrap_or_else(|| "000000".into());
    let styles = xfs.iter().map(|&(font, fill)| (
        fills.get(fill).cloned().flatten().filter(|f| color::marks_fill(f)),
        fonts.get(font).cloned().flatten().filter(|c| color::marks_text(c, &body)),
    )).collect();
    let rich = rich_strings(&mut zip, &theme, &indexed, &body);
    let sheets = sheet_parts(&mut zip);
    Some(Book { styles, rich, sheets })
}

/// Shared strings written in pieces (`<r>`) where a piece has its own color.
fn rich_strings(zip: &mut Zip, theme: &[String], indexed: &[String], body: &str) -> HashMap<usize, Vec<(String, Option<String>)>> {
    let mut out = HashMap::new();
    let Ok(Some(xml)) = ooxml::part(zip, "xl/sharedStrings.xml") else { return out };
    let mut reader = quick_xml::Reader::from_str(&xml);
    let (mut index, mut pieces, mut piece, mut piece_color, mut in_t, mut in_run, mut colored) = (0usize, Vec::new(), String::new(), None, false, false, false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                "si" => { pieces = Vec::new(); colored = false }
                "r" => { in_run = true; piece.clear(); piece_color = None }
                "t" => in_t = true,
                "rPh" => in_t = false,
                _ => {}
            },
            Ok(Event::Empty(e)) if in_run && local(e.name().as_ref()) == "color" => {
                piece_color = resolve(&e, theme, indexed).filter(|c| color::marks_text(c, body));
            }
            Ok(Event::Text(t)) if in_t => piece.push_str(&t),
            Ok(Event::GeneralRef(r)) if in_t => {
                if let Some(c) = ooxml::entity(&r) { piece.push(c) }
            }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                "t" => in_t = false,
                "r" => {
                    in_run = false;
                    colored |= piece_color.is_some();
                    pieces.push((std::mem::take(&mut piece), piece_color.take()));
                }
                "si" => {
                    if colored { out.insert(index, std::mem::take(&mut pieces)); }
                    index += 1;
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// The protection a workbook keeps: whether its structure is locked
/// (`workbookProtection`) and the sheets locked against editing
/// (`sheetProtection`).
pub fn protection(bytes: &[u8]) -> (bool, std::collections::HashSet<String>) {
    let Ok(mut zip) = ooxml::open(bytes) else { return (false, Default::default()) };
    let locked = |xml: &str, tag: &str| xml.contains(&format!("<{tag}")) || xml.contains(&format!(":{tag}"));
    let book = matches!(ooxml::part(&mut zip, "xl/workbook.xml"), Ok(Some(x)) if locked(&x, "workbookProtection"));
    let mut sheets = std::collections::HashSet::new();
    for (name, path) in sheet_parts(&mut zip) {
        if matches!(ooxml::part(&mut zip, &path), Ok(Some(x)) if locked(&x, "sheetProtection")) { sheets.insert(name); }
    }
    (book, sheets)
}

/// Sheet name → part, from the workbook and its relationships.
fn sheet_parts(zip: &mut Zip) -> HashMap<String, String> {
    let mut targets = HashMap::new();
    if let Ok(Some(rels)) = ooxml::part(zip, "xl/_rels/workbook.xml.rels") {
        let mut reader = quick_xml::Reader::from_str(&rels);
        loop {
            match reader.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if local(e.name().as_ref()) == "Relationship" => {
                    if let (Some(id), Some(target)) = (attr(&e, "Id"), attr(&e, "Target")) {
                        let path = if let Some(abs) = target.strip_prefix('/') { abs.to_string() } else { format!("xl/{target}") };
                        targets.insert(id, path);
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    let mut out = HashMap::new();
    if let Ok(Some(book)) = ooxml::part(zip, "xl/workbook.xml") {
        let mut reader = quick_xml::Reader::from_str(&book);
        loop {
            match reader.read_event() {
                Ok(Event::Empty(e)) | Ok(Event::Start(e)) if local(e.name().as_ref()) == "sheet" => {
                    if let (Some(name), Some(id)) = (attr(&e, "name"), ooxml::qattr(&e, ":id")) {
                        if let Some(path) = targets.get(&id) { out.insert(name, path.clone()); }
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    out
}

/// "B3" → (row 2, column 1), zero-based.
fn position(reference: &str) -> Option<(u32, u32)> {
    let letters: String = reference.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let row: u32 = reference[letters.len()..].parse().ok()?;
    let column = letters.to_ascii_uppercase().bytes().try_fold(0u32, |acc, b| acc.checked_mul(26)?.checked_add((b - b'A' + 1) as u32))?;
    Some((row.checked_sub(1)?, column.checked_sub(1)?))
}

impl Book {
    /// The cells of a sheet that show a color, by (row, column) zero-based.
    pub fn sheet(&self, bytes: &[u8], name: &str) -> HashMap<(u32, u32), Cell> {
        let mut out = HashMap::new();
        let Some(path) = self.sheets.get(name) else { return out };
        let Ok(mut zip) = ooxml::open(bytes) else { return out };
        let Ok(Some(xml)) = ooxml::part(&mut zip, path) else { return out };
        let mut reader = quick_xml::Reader::from_str(&xml);
        let (mut at, mut style, mut shared, mut in_v, mut value) = (None, 0usize, false, false, String::new());
        let finish = |at: Option<(u32, u32)>, style: usize, shared: bool, value: &str, out: &mut HashMap<(u32, u32), Cell>| {
            let Some(at) = at else { return };
            let (fill, color) = self.styles.get(style).cloned().unwrap_or_default();
            let pieces = if shared { value.trim().parse::<usize>().ok().and_then(|i| self.rich.get(&i)).cloned().unwrap_or_default() } else { Vec::new() };
            let cell = Cell { fill, color, pieces };
            if cell != Cell::default() { out.insert(at, cell); }
        };
        loop {
            match reader.read_event() {
                // A cell with no value (only a style: a gray fill across a row).
                Ok(Event::Empty(e)) if local(e.name().as_ref()) == "c" => {
                    let position_of = attr(&e, "r").and_then(|r| position(&r));
                    let style_of = attr(&e, "s").and_then(|v| v.parse().ok()).unwrap_or(0);
                    finish(position_of, style_of, false, "", &mut out);
                }
                Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                    "c" => {
                        at = attr(&e, "r").and_then(|r| position(&r));
                        style = attr(&e, "s").and_then(|v| v.parse().ok()).unwrap_or(0);
                        shared = attr(&e, "t").as_deref() == Some("s");
                        value.clear();
                    }
                    "v" => in_v = true,
                    _ => {}
                },
                Ok(Event::Text(t)) if in_v => value.push_str(&t),
                Ok(Event::End(e)) => match local(e.name().as_ref()) {
                    "v" => in_v = false,
                    "c" => finish(at.take(), style, shared, &value, &mut out),
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        out
    }
}

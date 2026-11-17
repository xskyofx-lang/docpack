//! Spreadsheets: one section and one CSV table per sheet. A cell keeps its
//! value; a formula that was never calculated (no stored value) is written as
//! `=FORMULA` so it is not lost.
use crate::{CellColor, Document, Mark, Section, Table};
use calamine::Reader;
use std::io::Cursor;

pub fn extract(bytes: &[u8], ext: &str) -> Result<Document, String> {
    if ext != "xls" {
        let zip = crate::ooxml::open(bytes)?;
        // A workbook Excel saved always has these parts; a file with only a
        // sheet (written by a script) is not one, and the reader's own message
        // ("File not found '_rels/.rels'") did not say so.
        if matches!(ext, "xlsx" | "xlsm") {
            let names = crate::ooxml::names(&zip);
            let missing: Vec<&str> = ["[Content_Types].xml", "_rels/.rels", "xl/workbook.xml"].into_iter().filter(|p| !names.iter().any(|n| n == p)).collect();
            if !missing.is_empty() {
                return Err(format!("No es un libro de Excel completo: le falta {} (Excel lo crea al guardar). Ábrelo y guárdalo de nuevo con Excel.", missing.join(", ")));
            }
        }
    }
    // Each format by its own reader: the automatic detection fails on a
    // workbook whose parts are stored without compression.
    let cursor = || Cursor::new(bytes.to_vec());
    let bad = |e: String| format!("Hoja de cálculo ilegible: {e}");
    let mut book: calamine::Sheets<Cursor<Vec<u8>>> = match ext {
        "xls" => calamine::Sheets::Xls(calamine::Xls::new(cursor()).map_err(|e| bad(e.to_string()))?),
        "xlsb" => calamine::Sheets::Xlsb(calamine::Xlsb::new(cursor()).map_err(|e| bad(e.to_string()))?),
        "ods" => calamine::Sheets::Ods(calamine::Ods::new(cursor()).map_err(|e| bad(e.to_string()))?),
        _ => calamine::Sheets::Xlsx(calamine::Xlsx::new(cursor()).map_err(|e| bad(e.to_string()))?),
    };
    let names = book.sheet_names().to_vec();
    if names.is_empty() { return Err("Libro sin hojas".into()) }
    let mut doc = Document { kind: "xlsx", pages: Some(names.len() as u32), ..Default::default() };
    // Spaces after a cell's text are padding (fixed-width exports fill every
    // cell to its column's width): they carry nothing and, in a real book,
    // were most of its CSV. Spaces before the text stay: they indent.
    let mut padding = 0usize;
    // Colors only Office Open XML keeps where they can be read (.xlsx, .xlsm).
    let colors = if matches!(ext, "xlsx" | "xlsm") { crate::xlsx_color::read(bytes) } else { None };
    for (n, name) in names.iter().enumerate() {
        let values = book.worksheet_range(name).map_err(|e| format!("Hoja {name}: {e}"))?;
        let formulas = book.worksheet_formula(name).ok();
        let mut grid = std::collections::BTreeMap::<(u32, u32), String>::new();
        let (vr, vc) = values.start().unwrap_or((0, 0));
        for (r, c, cell) in values.used_cells() {
            let full = cell.to_string();
            let v = full.trim_end();
            padding += full.len() - v.len();
            if !v.is_empty() { grid.insert((vr + r as u32, vc + c as u32), v.to_string()); }
        }
        // Every formula of the sheet: a cell without a stored value shows it in
        // the CSV; all of them go beside the table in Tnn.formulas.json.
        let mut sheet_formulas = std::collections::BTreeMap::<(u32, u32), String>::new();
        if let Some(f) = &formulas {
            let (fr, fc) = f.start().unwrap_or((0, 0));
            for (r, c, x) in f.used_cells() {
                if x.is_empty() { continue }
                let at = (fr + r as u32, fc + c as u32);
                grid.entry(at).or_insert_with(|| format!("={x}"));
                sheet_formulas.insert(at, format!("={x}"));
            }
        }
        let mut section = Section { title: format!("Hoja: {name}"), level: 1, page_start: Some(n as u32 + 1), page_end: Some(n as u32 + 1), ..Default::default() };
        if grid.is_empty() {
            section.text = "Hoja vacía.".into();
        } else {
            let (r0, r1) = (grid.keys().map(|k| k.0).min().unwrap(), grid.keys().map(|k| k.0).max().unwrap());
            let (c0, c1) = (grid.keys().map(|k| k.1).min().unwrap(), grid.keys().map(|k| k.1).max().unwrap());
            let rows: Vec<Vec<String>> = (r0..=r1).map(|r| (c0..=c1).map(|c| grid.get(&(r, c)).cloned().unwrap_or_default()).collect()).collect();
            let id = doc.tables.len();
            section.text = format!("{} filas × {} columnas desde {}{}.\n\n[Tabla T{:02} → tablas/T{:02}.csv]\n", rows.len(), c1 - c0 + 1, column(c0), r0 + 1, id + 1, id + 1);
            section.tables.push(id);
            // The cells the sheet marks with color, at their place in the CSV.
            let mut marked = Vec::new();
            if let Some(book) = &colors {
                let mut cells: Vec<_> = book.sheet(bytes, name).into_iter().filter(|((r, c), _)| (r0..=r1).contains(r) && (c0..=c1).contains(c)).collect();
                cells.sort_by_key(|(at, _)| *at);
                for ((r, c), cell) in cells {
                    let (row, column) = ((r - r0) as usize, (c - c0) as usize);
                    let value = &rows[row][column];
                    let mut text = Vec::new();
                    if !cell.pieces.is_empty() {
                        for (piece, color) in cell.pieces {
                            if let (Some(k), false) = (color, piece.trim().is_empty()) { text.push(Mark { text: piece.trim_end().to_string(), color: Some(k), fill: None }) }
                        }
                    } else if let (Some(k), false) = (cell.color, value.is_empty()) {
                        text.push(Mark { text: value.clone(), color: Some(k), fill: None });
                    }
                    if let Some(f) = &cell.fill { doc.colors.add("fondo", f, value) }
                    for m in &text { if let Some(k) = &m.color { doc.colors.add("texto", k, &m.text) } }
                    if cell.fill.is_some() || !text.is_empty() { marked.push(CellColor { row, column, fill: cell.fill, text }) }
                }
            }
            let formulas_in_table = sheet_formulas.into_iter()
                .filter(|((r, c), _)| (r0..=r1).contains(r) && (c0..=c1).contains(c))
                .map(|((r, c), f)| ((r - r0) as usize, (c - c0) as usize, f)).collect();
            let (header_row, heading) = header_below_titles(&rows);
            let title = if heading.is_empty() { name.clone() } else { format!("{name} · {}", heading.join(" · ")) };
            doc.tables.push(Table { title, page: Some(n as u32 + 1), pages: Vec::new(), rows, colors: marked, formulas: formulas_in_table, header_row, ..Default::default() });
        }
        doc.sections.push(section);
    }
    if padding > 0 {
        doc.notes.push(format!("Se quitaron {padding} espacios de relleno al final de las celdas (exportaciones de ancho fijo). Los espacios al inicio, que marcan sangría, se conservan."));
    }
    Ok(doc)
}

/// A sheet often carries its title above the table ("Inventory, 4th
/// quarter" in a merged row) and a blank line: rows with one text at most.
/// The header is the first row with two or more cells filled after them; the
/// title lines are returned to name the table. A sheet that starts with its
/// header (or is one column wide) keeps row 0.
fn header_below_titles(rows: &[Vec<String>]) -> (usize, Vec<String>) {
    let filled = |r: &Vec<String>| r.iter().filter(|c| !c.trim().is_empty()).count();
    match rows.iter().position(|r| filled(r) >= 2) {
        Some(at) if at > 0 => (at, rows[..at].iter().flat_map(|r| r.iter().filter(|c| !c.trim().is_empty()).cloned()).collect()),
        _ => (0, Vec::new()),
    }
}

fn column(mut index: u32) -> String {
    let mut s = String::new();
    index += 1;
    while index > 0 { let r = (index - 1) % 26; s.insert(0, (b'A' + r as u8) as char); index = (index - 1) / 26; }
    s
}

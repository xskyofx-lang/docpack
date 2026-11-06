//! Spreadsheets: one section and one CSV table per sheet. A cell keeps its
//! value; a formula that was never calculated (no stored value) is written as
//! `=FORMULA` so it is not lost.
use crate::{Document, Section, Table};
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
    for (n, name) in names.iter().enumerate() {
        let values = book.worksheet_range(name).map_err(|e| format!("Hoja {name}: {e}"))?;
        let formulas = book.worksheet_formula(name).ok();
        let mut grid = std::collections::BTreeMap::<(u32, u32), String>::new();
        let (vr, vc) = values.start().unwrap_or((0, 0));
        for (r, c, cell) in values.used_cells() {
            let v = cell.to_string();
            if !v.is_empty() { grid.insert((vr + r as u32, vc + c as u32), v); }
        }
        if let Some(f) = &formulas {
            let (fr, fc) = f.start().unwrap_or((0, 0));
            for (r, c, x) in f.used_cells() {
                if !x.is_empty() { grid.entry((fr + r as u32, fc + c as u32)).or_insert_with(|| format!("={x}")); }
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
            doc.tables.push(Table { title: name.clone(), page: Some(n as u32 + 1), rows });
        }
        doc.sections.push(section);
    }
    Ok(doc)
}

fn column(mut index: u32) -> String {
    let mut s = String::new();
    index += 1;
    while index > 0 { let r = (index - 1) % 26; s.insert(0, (b'A' + r as u8) as char); index = (index - 1) / 26; }
    s
}

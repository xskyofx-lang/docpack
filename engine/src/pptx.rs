//! PowerPoint: one section per slide, in the order the presentation shows
//! them (`p:sldIdLst` through its relationships; file numbers are only the
//! fallback). Text lines go to the section; slide tables become CSV.
use crate::ooxml::{self, entity, local, qattr, Skip};
use crate::{Document, Section, Table};
use quick_xml::events::Event;

fn numbered(zip: &ooxml::Zip) -> Vec<String> {
    let mut found: Vec<(u32, String)> = ooxml::names(zip).into_iter()
        .filter_map(|n| n.strip_prefix("ppt/slides/slide").and_then(|r| r.strip_suffix(".xml")).and_then(|d| d.parse().ok()).map(|k| (k, n.clone())))
        .collect();
    found.sort();
    found.into_iter().map(|(_, n)| n).collect()
}

fn slide_order(zip: &mut ooxml::Zip) -> Result<Vec<String>, String> {
    let (Some(presentation), Some(rels)) = (ooxml::part(zip, "ppt/presentation.xml")?, ooxml::part(zip, "ppt/_rels/presentation.xml.rels")?) else { return Ok(numbered(zip)) };
    let mut targets = std::collections::HashMap::new();
    let mut reader = quick_xml::Reader::from_str(&rels);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == "Relationship" => {
                if let (Some(id), Some(target)) = (ooxml::attr(&e, "Id"), ooxml::attr(&e, "Target")) { targets.insert(id, target); }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let names = ooxml::names(zip);
    let mut order = Vec::new();
    let mut reader = quick_xml::Reader::from_str(&presentation);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == "sldId" => {
                if let Some(target) = qattr(&e, ":id").and_then(|rid| targets.get(&rid).cloned()) {
                    let path = target.trim_start_matches('/');
                    let path = if path.starts_with("ppt/") { path.to_string() } else { format!("ppt/{path}") };
                    if names.contains(&path) { order.push(path); }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    Ok(if order.is_empty() { numbered(zip) } else { order })
}

pub fn extract(bytes: &[u8]) -> Result<Document, String> {
    let mut zip = ooxml::open(bytes)?;
    let order = slide_order(&mut zip)?;
    if order.is_empty() { return Err("PPTX sin diapositivas reconocibles".into()) }
    let mut doc = Document { kind: "pptx", pages: Some(order.len() as u32), ..Default::default() };
    for (n, name) in order.iter().enumerate() {
        let number = n as u32 + 1;
        let Some(xml) = ooxml::part(&mut zip, name)? else { continue };
        let mut reader = quick_xml::Reader::from_str(&xml);
        let mut skip = Skip::new();
        let (mut lines, mut line, mut inside) = (Vec::<String>::new(), String::new(), false);
        let (mut in_table, mut cell, mut row, mut rows) = (0usize, String::new(), Vec::<String>::new(), Vec::<Vec<String>>::new());
        let mut tables: Vec<Vec<Vec<String>>> = Vec::new();
        loop {
            let event = reader.read_event();
            if skip.skips(&event) { continue }
            match &event {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                    "t" if matches!(event, Ok(Event::Start(_))) => inside = true,
                    "tbl" if matches!(event, Ok(Event::Start(_))) => in_table += 1,
                    "tab" => line.push('\t'),
                    "br" => line.push(if in_table > 0 { ' ' } else { '\n' }),
                    _ => {}
                },
                Ok(Event::Text(t)) if inside => line.push_str(t),
                Ok(Event::GeneralRef(r)) if inside => { if let Some(c) = entity(r) { line.push(c) } }
                Ok(Event::End(e)) => match local(e.name().as_ref()) {
                    "t" => inside = false,
                    "p" => {
                        let text = std::mem::take(&mut line);
                        if text.trim().is_empty() { continue }
                        if in_table > 0 { if !cell.is_empty() { cell.push_str(" / ") } cell.push_str(text.trim()) } else { lines.push(text.trim().to_string()) }
                    }
                    "tc" if in_table > 0 => row.push(std::mem::take(&mut cell)),
                    "tr" if in_table > 0 => { if row.iter().any(|c| !c.trim().is_empty()) { rows.push(std::mem::take(&mut row)) } else { row.clear() } }
                    "tbl" => { in_table = in_table.saturating_sub(1); if in_table == 0 && !rows.is_empty() { tables.push(std::mem::take(&mut rows)) } }
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => return Err(format!("XML de diapositiva inválido ({name}): {e}")),
                _ => {}
            }
        }
        let title = lines.first().cloned().unwrap_or_default();
        let mut section = Section {
            title: if title.is_empty() { format!("Diapositiva {number}") } else { format!("Diapositiva {number}: {}", title.chars().take(80).collect::<String>()) },
            level: 1, page_start: Some(number), page_end: Some(number), ..Default::default()
        };
        section.text = lines.iter().skip(1).map(|l| format!("{l}\n\n")).collect();
        for rows in tables {
            let id = doc.tables.len();
            section.text.push_str(&format!("[Tabla T{:02} → tablas/T{:02}.csv]\n\n", id + 1, id + 1));
            section.tables.push(id);
            doc.tables.push(Table { title: section.title.clone(), page: Some(number), rows });
        }
        doc.sections.push(section);
    }
    Ok(doc)
}

//! Word: one section per heading. A heading is what the document declares
//! (a style with an outline level, followed through `basedOn`, the built-in
//! "Title" style, or a paragraph's own outline level), not a word in a style
//! name. Top-level tables become CSV; a nested table stays inside its cell.
use crate::ooxml::{self, attr, entity, local, Skip};
use crate::{Document, Section, Table};
use quick_xml::events::Event;
use std::collections::HashMap;

/// Style id → heading level (0 title, 1-9 heading).
fn heading_styles(zip: &mut ooxml::Zip) -> Result<HashMap<String, u8>, String> {
    let Some(xml) = ooxml::part(zip, "word/styles.xml")? else {
        // Without styles.xml Word uses its built-in ids.
        let mut builtin: HashMap<String, u8> = (1..=9).map(|n| (format!("Heading{n}"), n as u8)).collect();
        builtin.insert("Title".into(), 0);
        return Ok(builtin);
    };
    let mut styles: HashMap<String, (Option<String>, Option<u8>, bool)> = HashMap::new();
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut current: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                "style" => { current = attr(&e, "styleId"); if let Some(id) = &current { styles.entry(id.clone()).or_default(); } }
                "basedOn" => if let Some(id) = &current { styles.entry(id.clone()).or_default().0 = attr(&e, "val"); },
                "outlineLvl" => if let Some(id) = &current { styles.entry(id.clone()).or_default().1 = attr(&e, "val").and_then(|v| v.parse::<u8>().ok()).filter(|l| *l < 9); },
                // The built-in names are fixed by the standard ("Title",
                // "heading 1".."heading 9", in English whatever the language):
                // Word gives them their outline level even when styles.xml
                // does not write it down.
                "name" => if let Some(id) = &current {
                    let name = attr(&e, "val").unwrap_or_default();
                    let entry = styles.entry(id.clone()).or_default();
                    if name == "Title" { entry.2 = true; }
                    if entry.1.is_none() { entry.1 = name.to_ascii_lowercase().strip_prefix("heading ").and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=9).contains(n)).map(|n| n - 1); }
                },
                _ => {}
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == "style" => current = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let mut out = HashMap::new();
    for id in styles.keys() {
        let (mut at, mut hops) = (Some(id.clone()), 0);
        while let Some(style) = at.as_ref().and_then(|s| styles.get(s)) {
            if style.2 { out.insert(id.clone(), 0); break }
            if let Some(level) = style.1 { out.insert(id.clone(), level + 1); break }
            hops += 1;
            if hops > styles.len() { break }
            at = style.0.clone();
        }
    }
    Ok(out)
}

#[derive(Default)]
struct Para { text: String, style: String, own_level: Option<u8>, list: bool, list_level: usize, images: Vec<String> }

/// Relationship id → part path (word/media/image1.png) for the document's images.
fn image_targets(zip: &mut ooxml::Zip) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    let Some(xml) = ooxml::part(zip, "word/_rels/document.xml.rels")? else { return Ok(out) };
    let mut reader = quick_xml::Reader::from_str(&xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == "Relationship" => {
                if let (Some(id), Some(target)) = (attr(&e, "Id"), attr(&e, "Target")) {
                    if attr(&e, "TargetMode").as_deref() != Some("External") { out.insert(id, format!("word/{}", target.trim_start_matches("./"))); }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    Ok(out)
}

pub fn extract(bytes: &[u8]) -> Result<Document, String> {
    let mut zip = ooxml::open(bytes)?;
    let xml = ooxml::part(&mut zip, "word/document.xml")?.ok_or("DOCX sin word/document.xml")?;
    let levels = heading_styles(&mut zip)?;
    let images = image_targets(&mut zip)?;
    let mut doc = Document { kind: "docx", ..Default::default() };
    // The same image file used twice is one figure (falla 7).
    let mut figure_of: HashMap<String, usize> = HashMap::new();
    // Symbol-font characters (Wingdings, Symbol) have no Unicode value to give.
    let mut font_symbols = 0usize;
    doc.notes.push("Word no guarda páginas: las secciones no llevan número de página.".into());
    let mut sections = vec![Section { title: "Inicio".into(), ..Default::default() }];
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut skip = Skip::new();
    let mut para = Para::default();
    let mut outer: Vec<Para> = Vec::new();
    let mut open = 0usize;
    let mut in_text = false;
    let (mut depth, mut cell, mut row, mut rows) = (0usize, String::new(), Vec::<String>::new(), Vec::<Vec<String>>::new());
    loop {
        let event = reader.read_event();
        if skip.skips(&event) { continue }
        match &event {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let empty = matches!(event, Ok(Event::Empty(_)));
                match local(e.name().as_ref()) {
                    // A text box holds whole paragraphs inside a paragraph: the
                    // outer one waits while the inner ones are read.
                    "p" if !empty => { if open > 0 { outer.push(std::mem::take(&mut para)); } open += 1; para = Para::default(); }
                    "pStyle" => para.style = attr(e, "val").unwrap_or_default(),
                    "outlineLvl" => para.own_level = attr(e, "val").and_then(|v| v.parse::<u8>().ok()).filter(|l| *l < 9).map(|l| l + 1),
                    "numPr" => para.list = true,
                    "ilvl" => para.list_level = attr(e, "val").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(8),
                    // A symbol inserted as such: Word keeps its Unicode value in the
                    // `mc:Choice` (w16se:symEx) and the plain text only in the
                    // `mc:Fallback` DocPack skips, so it is read from here (falla 1).
                    "symEx" | "sym" => match attr(e, "char").and_then(|c| u32::from_str_radix(&c, 16).ok()) {
                        Some(code) if (0xF000..=0xF0FF).contains(&code) => font_symbols += 1,
                        Some(code) => if let Some(c) = char::from_u32(code) { para.text.push(c) },
                        None => {}
                    },
                    "t" if !empty => in_text = true,
                    "tab" => para.text.push('\t'),
                    // A line break stays a line break, also inside a table cell: an
                    // apart condition must not run into the next one (falla 2).
                    "br" | "cr" => para.text.push('\n'),
                    "tbl" if !empty => depth += 1,
                    "blip" => if let Some(target) = crate::ooxml::qattr(e, ":embed").and_then(|id| images.get(&id).cloned()) { para.images.push(target) },
                    "tr" if !empty && depth == 1 => row.clear(),
                    "tc" if !empty && depth == 1 => cell.clear(),
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if in_text => para.text.push_str(t),
            Ok(Event::GeneralRef(r)) if in_text => { if let Some(c) = entity(r) { para.text.push(c) } }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                "t" => in_text = false,
                "p" => {
                    let text = para.text.trim_end().to_string();
                    if depth > 0 {
                        if !text.trim().is_empty() { if !cell.is_empty() { cell.push('\n') } cell.push_str(text.trim()); }
                    } else if !text.trim().is_empty() {
                        let level = para.own_level.or_else(|| levels.get(&para.style).copied());
                        match level {
                            Some(l) => sections.push(Section { title: text.trim().replace('\n', " "), level: l.max(1), ..Default::default() }),
                            None => {
                                let s = sections.last_mut().unwrap();
                                if para.list {
                                    // The list level is kept as indentation (falla 3).
                                    s.text.push_str(&format!("{}- {}\n", "  ".repeat(para.list_level), text.trim().replace('\n', " ")));
                                } else {
                                    // A paragraph after a list starts after a blank line, or
                                    // Markdown joins it to the last item (falla 6).
                                    let after_item = s.text.trim_end_matches('\n').rsplit('\n').next().is_some_and(|l| l.trim_start().starts_with("- "));
                                    if after_item && !s.text.ends_with("\n\n") { s.text.push('\n'); }
                                    // The lines of one text box stay one block (falla 9).
                                    if open > 1 { s.text.push_str(&format!("{text}\n")) } else { s.text.push_str(&format!("{text}\n\n")) }
                                }
                            }
                        }
                    }
                    // Images of this paragraph go to figuras/, referenced where they were.
                    for target in std::mem::take(&mut para.images) {
                        if let Some(&id) = figure_of.get(&target) {
                            let section = sections.last_mut().unwrap();
                            let file = format!("figuras/F{:02}.{}", id + 1, doc.figures[id].extension);
                            section.text.push_str(&format!("[Figura F{:02} → {file}]\n\n", id + 1));
                            if !section.figures.contains(&id) { section.figures.push(id); }
                            continue;
                        }
                        let Ok(mut file) = zip.by_name(&target) else { continue };
                        let mut bytes = Vec::new();
                        if std::io::Read::read_to_end(&mut file, &mut bytes).is_err() || bytes.is_empty() { continue }
                        let extension = target.rsplit('.').next().unwrap_or("bin").to_ascii_lowercase();
                        let section = sections.last_mut().unwrap();
                        let id = doc.figures.len();
                        section.text.push_str(&format!("[Figura F{:02} → figuras/F{:02}.{extension}]\n\n", id + 1, id + 1));
                        section.figures.push(id);
                        doc.figures.push(crate::Figure { section: section.title.clone(), extension, bytes });
                        figure_of.insert(target, id);
                    }
                    open = open.saturating_sub(1);
                    para = outer.pop().unwrap_or_default();
                }
                "tc" if depth == 1 => row.push(std::mem::take(&mut cell)),
                "tr" if depth == 1 => { if row.iter().any(|c| !c.trim().is_empty()) { rows.push(std::mem::take(&mut row)) } else { row.clear() } }
                "tbl" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 && !rows.is_empty() {
                        let section = sections.last_mut().unwrap();
                        let id = doc.tables.len();
                        if !section.text.is_empty() && !section.text.ends_with("\n\n") { section.text.push('\n'); }
                        section.text.push_str(&format!("[Tabla T{:02} → tablas/T{:02}.csv]\n\n", id + 1, id + 1));
                        section.tables.push(id);
                        doc.tables.push(Table { title: section.title.clone(), page: None, rows: std::mem::take(&mut rows) });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML de Word inválido: {e}")),
            _ => {}
        }
    }
    // Headers and footers are document text too (a code, a date, a name).
    let mut extra = String::new();
    for name in ooxml::names(&zip).into_iter().filter(|n| (n.starts_with("word/header") || n.starts_with("word/footer")) && n.ends_with(".xml")) {
        if let Some(xml) = ooxml::part(&mut zip, &name)? {
            let text = plain_text(&xml);
            if !text.trim().is_empty() && !extra.contains(text.trim()) { extra.push_str(text.trim()); extra.push('\n'); }
        }
    }
    if !extra.is_empty() { sections.push(Section { title: "Encabezados y pies de página".into(), level: 1, text: extra, every_page: true, ..Default::default() }); }
    // Header and footer images are logos repeated on every page: counted, not
    // exported, so the figures are the document's own (falla 8).
    let header_images = ooxml::names(&zip).iter().filter(|n| n.starts_with("word/_rels/header") || n.starts_with("word/_rels/footer"))
        .filter_map(|n| ooxml::part(&mut zip, n).ok().flatten()).map(|x| x.matches("relationships/image").count()).sum::<usize>();
    if header_images > 0 { doc.notes.push(format!("{header_images} imagen(es) de encabezado o pie de página (logos) no se exportan.")); }
    if font_symbols > 0 { doc.notes.push(format!("{font_symbols} símbolo(s) de fuentes Symbol/Wingdings sin equivalente Unicode no se pudieron convertir.")); }
    doc.sections = sections.into_iter().filter(|s| !(s.level == 0 && s.text.trim().is_empty() && s.tables.is_empty())).collect();
    Ok(doc)
}

/// The text of a header or footer part, paragraph per line.
fn plain_text(xml: &str) -> String {
    let mut reader = quick_xml::Reader::from_str(xml);
    let (mut out, mut line, mut inside) = (String::new(), String::new(), false);
    let mut skip = Skip::new();
    loop {
        let event = reader.read_event();
        if skip.skips(&event) { continue }
        match &event {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == "t" => inside = true,
            Ok(Event::Text(t)) if inside => line.push_str(t),
            Ok(Event::End(e)) => match local(e.name().as_ref()) { "t" => inside = false, "p" => { if !line.trim().is_empty() { out.push_str(line.trim()); out.push('\n'); } line.clear(); } _ => {} },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

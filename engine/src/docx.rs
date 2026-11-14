//! Word: one section per heading. A heading is what the document declares
//! (a style with an outline level, followed through `basedOn`, the built-in
//! "Title" style, or a paragraph's own outline level), not a word in a style
//! name. Top-level tables become CSV; a nested table stays inside its cell.
use crate::ooxml::{self, attr, entity, local, Skip};
use crate::{color, CellColor, Document, Mark, Section, Table};
use quick_xml::events::Event;
use std::collections::HashMap;

/// Style id → heading level (0 title, 1-9 heading), and the ids of caption
/// styles (the built-in "caption", and styles based on it).
fn heading_styles(zip: &mut ooxml::Zip) -> Result<(HashMap<String, u8>, std::collections::HashSet<String>), String> {
    let Some(xml) = ooxml::part(zip, "word/styles.xml")? else {
        // Without styles.xml Word uses its built-in ids.
        let mut builtin: HashMap<String, u8> = (1..=9).map(|n| (format!("Heading{n}"), n as u8)).collect();
        builtin.insert("Title".into(), 0);
        return Ok((builtin, ["Caption".to_string()].into()));
    };
    let mut styles: HashMap<String, (Option<String>, Option<u8>, bool)> = HashMap::new();
    let mut captions = std::collections::HashSet::new();
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
                    if name.eq_ignore_ascii_case("caption") { captions.insert(id.clone()); }
                    if entry.1.is_none() { entry.1 = name.to_ascii_lowercase().strip_prefix("heading ").and_then(|n| n.parse::<u8>().ok()).filter(|n| (1..=9).contains(n)).map(|n| n - 1); }
                },
                _ => {}
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == "style" => current = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    // A style based on a caption style is a caption too.
    for id in styles.keys() {
        let (mut at, mut hops) = (styles.get(id).and_then(|s| s.0.clone()), 0);
        while let Some(base) = at {
            if captions.contains(&base) { captions.insert(id.clone()); break }
            hops += 1;
            if hops > styles.len() { break }
            at = styles.get(&base).and_then(|s| s.0.clone());
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
    Ok((out, captions))
}

#[derive(Default)]
struct Para { text: String, style: String, own_level: Option<u8>, list: bool, list_level: usize, images: Vec<String>, numbered_caption: bool,
    /// Byte ranges of `text` in a color (or on a highlight) that marks them.
    marks: Vec<(usize, usize, Option<String>, Option<String>)>,
    /// The paragraph's own shading.
    fill: Option<String> }

/// The colors styles give their text: style id → (color, highlight or
/// shading), followed through `basedOn`; and the document's body color (the
/// default paragraph style's, else the document default's, else black).
struct StyleColors { by_id: HashMap<String, (Option<String>, Option<String>)>, body: String }

fn style_colors(zip: &mut ooxml::Zip) -> Result<StyleColors, String> {
    let mut own: HashMap<String, (Option<String>, Option<String>, Option<String>)> = HashMap::new();
    let (mut default_color, mut default_para) = (None, None);
    if let Some(xml) = ooxml::part(zip, "word/styles.xml")? {
        let mut reader = quick_xml::Reader::from_str(&xml);
        let (mut current, mut in_defaults, mut in_ppr) = (None::<String>, false, false);
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                    "rPrDefault" => in_defaults = true,
                    "style" => {
                        current = attr(&e, "styleId");
                        if attr(&e, "type").as_deref() == Some("paragraph") && attr(&e, "default").as_deref() == Some("1") { default_para = current.clone() }
                    }
                    "pPr" => in_ppr = true,
                    "basedOn" => if let Some(id) = &current { own.entry(id.clone()).or_default().0 = attr(&e, "val") },
                    "color" if !in_ppr => {
                        let c = attr(&e, "val").and_then(|v| color::normal(&v));
                        if let Some(id) = &current { own.entry(id.clone()).or_default().1 = c } else if in_defaults { default_color = c }
                    }
                    "highlight" | "shd" if !in_ppr => if let Some(id) = &current {
                        let f = if local(e.name().as_ref()) == "highlight" { attr(&e, "val").and_then(|v| color::highlight(&v)) } else { attr(&e, "fill").and_then(|v| color::normal(&v)) };
                        if f.is_some() { own.entry(id.clone()).or_default().2 = f }
                    },
                    _ => {}
                },
                Ok(Event::End(e)) => match local(e.name().as_ref()) { "style" => current = None, "rPrDefault" => in_defaults = false, "pPr" => in_ppr = false, _ => {} },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    let mut by_id = HashMap::new();
    for id in own.keys() {
        let (mut c, mut f, mut at, mut hops) = (None, None, Some(id.clone()), 0);
        while let Some(style) = at.as_ref().and_then(|s| own.get(s)) {
            c = c.or_else(|| style.1.clone());
            f = f.or_else(|| style.2.clone());
            hops += 1;
            if hops > own.len() { break }
            at = style.0.clone();
        }
        by_id.insert(id.clone(), (c, f));
    }
    let body = default_para.and_then(|p| by_id.get(&p).and_then(|s| s.0.clone())).or(default_color).unwrap_or_else(|| "000000".into());
    Ok(StyleColors { by_id, body })
}

/// Appends text to a paragraph, marked when its color or highlight marks it.
fn append(para: &mut Para, text: &str, color: Option<String>, fill: Option<String>) {
    let start = para.text.len();
    para.text.push_str(text);
    if color.is_none() && fill.is_none() { return }
    let end = para.text.len();
    match para.marks.last_mut() {
        Some(last) if last.1 == start && last.2 == color && last.3 == fill => last.1 = end,
        _ => para.marks.push((start, end, color, fill)),
    }
}

/// The paragraph's marked pieces, trimmed, without the empty ones.
fn pieces(para: &Para) -> Vec<Mark> {
    para.marks.iter().filter_map(|(a, b, c, f)| {
        let text = para.text.get(*a..*b)?.split_whitespace().collect::<Vec<_>>().join(" ");
        (!text.is_empty()).then(|| Mark { text, color: c.clone(), fill: f.clone() })
    }).collect()
}

/// The paragraph's text with its marked pieces wrapped in HTML (Markdown
/// keeps it): `<span style="color:#FF0000">` for colored text, `<mark
/// style="background:#FFFF00">` for a highlight or a shaded paragraph.
fn rendered(para: &Para) -> String {
    let mut out = String::new();
    let mut at = 0;
    for (a, b, c, f) in &para.marks {
        let (Some(before), Some(piece)) = (para.text.get(at..*a), para.text.get(*a..*b)) else { continue };
        out.push_str(before);
        // Spaces stay outside the tags.
        let lead = piece.len() - piece.trim_start().len();
        let trail = piece.trim_end().len();
        if piece.trim().is_empty() { out.push_str(piece) } else { out.push_str(&piece[..lead]); out.push_str(&crate::color::wrap(&piece[lead..trail], c.as_deref(), f.as_deref())); out.push_str(&piece[trail..]) }
        at = *b;
    }
    out.push_str(para.text.get(at..).unwrap_or(""));
    match &para.fill { Some(f) if !out.trim().is_empty() => crate::color::wrap(out.trim_end(), None, Some(f)), _ => out }
}

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
    let (levels, caption_styles) = heading_styles(&mut zip)?;
    // A table's caption: a paragraph in a caption style, or numbered by a
    // SEQ field ("Tabla {SEQ Tabla}"), just above it, or just below a table
    // that had none above.
    let mut caption_above: Option<String> = None;
    let mut untitled: Option<usize> = None;
    let mut in_field = false;
    let images = image_targets(&mut zip)?;
    let styles = style_colors(&mut zip)?;
    let mut doc = Document { kind: "docx", ..Default::default() };
    // The run being read: its own color, highlight or shading, and style.
    let (mut in_run, mut in_ppr, mut in_tcpr) = (false, false, false);
    // A hyperlink's color is the link's style, not a mark.
    let mut in_link = 0usize;
    let (mut run_color, mut run_fill, mut run_style): (Option<String>, Option<String>, Option<String>) = (None, None, None);
    // The cell being read (depth 1): its fill and marked pieces; the colored
    // cells of the row and of the table.
    let (mut cell_fill, mut cell_marks): (Option<String>, Vec<Mark>) = (None, Vec::new());
    let (mut row_colors, mut table_colors): (Vec<CellColor>, Vec<CellColor>) = (Vec::new(), Vec::new());
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
                    "r" if !empty => { in_run = true; run_color = None; run_fill = None; run_style = None; }
                    "hyperlink" if !empty => in_link += 1,
                    "pPr" if !empty => in_ppr = true,
                    "tcPr" if !empty => in_tcpr = true,
                    "rStyle" if in_run => run_style = attr(e, "val"),
                    "color" if in_run => run_color = attr(e, "val").and_then(|v| color::normal(&v)),
                    "highlight" if in_run => run_fill = attr(e, "val").and_then(|v| color::highlight(&v)).or(run_fill.take()),
                    "shd" => {
                        let fill = attr(e, "fill").and_then(|v| color::normal(&v)).filter(|f| color::marks_fill(f));
                        if in_run { if run_fill.is_none() { run_fill = fill } }
                        else if in_tcpr && depth == 1 { cell_fill = fill }
                        else if in_ppr { para.fill = fill }
                    }
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
                    "instrText" if !empty => in_field = true,
                    "fldSimple" => if attr(e, "instr").is_some_and(|i| i.trim_start().starts_with("SEQ")) { para.numbered_caption = true },
                    "tab" => para.text.push('\t'),
                    // A line break stays a line break, also inside a table cell: an
                    // apart condition must not run into the next one (falla 2).
                    "br" | "cr" => para.text.push('\n'),
                    "tbl" if !empty => depth += 1,
                    "blip" => if let Some(target) = crate::ooxml::qattr(e, ":embed").and_then(|id| images.get(&id).cloned()) { para.images.push(target) },
                    "tr" if !empty && depth == 1 => { row.clear(); row_colors.clear(); }
                    "tc" if !empty && depth == 1 => { cell.clear(); cell_fill = None; cell_marks.clear(); }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if in_text => {
                // Its color: the run's own, else its character style's, else
                // its paragraph style's; a mark only when it is not the body's.
                let of = |s: &Option<String>| s.as_ref().and_then(|s| styles.by_id.get(s)).cloned().unwrap_or_default();
                let (rs, ps) = (of(&run_style), of(&Some(para.style.clone())));
                let c = run_color.clone().or(rs.0).or(ps.0).filter(|c| in_link == 0 && color::marks_text(c, &styles.body));
                let f = run_fill.clone().or(rs.1).filter(|f| color::marks_fill(f));
                append(&mut para, t, c, f);
            }
            Ok(Event::Text(t)) if in_field => { let code: &str = t; if code.trim_start().starts_with("SEQ") { para.numbered_caption = true } }
            Ok(Event::GeneralRef(r)) if in_text => { if let Some(c) = entity(r) { para.text.push(c) } }
            Ok(Event::End(e)) => match local(e.name().as_ref()) {
                "t" => in_text = false,
                "r" => in_run = false,
                "hyperlink" => in_link = in_link.saturating_sub(1),
                "pPr" => in_ppr = false,
                "tcPr" => in_tcpr = false,
                "instrText" => in_field = false,
                "p" => {
                    let text = para.text.trim_end().to_string();
                    if depth > 0 {
                        if !text.trim().is_empty() { if !cell.is_empty() { cell.push('\n') } cell.push_str(text.trim()); }
                        cell_marks.extend(pieces(&para));
                        if cell_fill.is_none() && depth == 1 { cell_fill = para.fill.clone() }
                    } else if !text.trim().is_empty() {
                        for m in pieces(&para) {
                            if let Some(c) = &m.color { doc.colors.add("texto", c, &m.text) }
                            if let Some(f) = &m.fill { doc.colors.add("resaltado", f, &m.text) }
                        }
                        if let Some(f) = &para.fill { doc.colors.add("fondo", f, text.trim()) }
                        let shown = rendered(&para);
                        let shown = shown.trim();
                        let level = para.own_level.or_else(|| levels.get(&para.style).copied());
                        let caption = level.is_none() && (para.numbered_caption || caption_styles.contains(&para.style));
                        let line = text.trim().replace('\n', " ");
                        match (caption, untitled.take()) {
                            (true, Some(t)) => doc.tables[t].title = line,
                            (true, None) => caption_above = Some(line),
                            (false, _) => caption_above = None,
                        }
                        match level {
                            Some(l) => sections.push(Section { title: text.trim().replace('\n', " "), level: l.max(1), ..Default::default() }),
                            None => {
                                let s = sections.last_mut().unwrap();
                                if para.list {
                                    // The list level is kept as indentation (falla 3).
                                    s.text.push_str(&format!("{}- {}\n", "  ".repeat(para.list_level), shown.replace('\n', " ")));
                                } else {
                                    // A paragraph after a list starts after a blank line, or
                                    // Markdown joins it to the last item (falla 6).
                                    let after_item = s.text.trim_end_matches('\n').rsplit('\n').next().is_some_and(|l| l.trim_start().starts_with("- "));
                                    if after_item && !s.text.ends_with("\n\n") { s.text.push('\n'); }
                                    // The lines of one text box stay one block (falla 9).
                                    if open > 1 { s.text.push_str(&format!("{shown}\n")) } else { s.text.push_str(&format!("{shown}\n\n")) }
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
                "tc" if depth == 1 => {
                    if cell_fill.is_some() || !cell_marks.is_empty() {
                        row_colors.push(CellColor { row: 0, column: row.len(), fill: cell_fill.take(), text: std::mem::take(&mut cell_marks) });
                    }
                    row.push(std::mem::take(&mut cell));
                }
                "tr" if depth == 1 => {
                    if row.iter().any(|c| !c.trim().is_empty()) {
                        for mut c in row_colors.drain(..) {
                            c.row = rows.len();
                            if let Some(f) = &c.fill { doc.colors.add("fondo", f, &row[c.column]) }
                            for m in &c.text {
                                if let Some(k) = &m.color { doc.colors.add("texto", k, &m.text) }
                                if let Some(k) = &m.fill { doc.colors.add("resaltado", k, &m.text) }
                            }
                            table_colors.push(c);
                        }
                        rows.push(std::mem::take(&mut row))
                    } else { row.clear(); row_colors.clear() }
                }
                "tbl" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 && !rows.is_empty() {
                        let section = sections.last_mut().unwrap();
                        let id = doc.tables.len();
                        // Its caption above, else (until one below) its section's title.
                        let title = match caption_above.take() { Some(c) => c, None => { untitled = Some(id); section.title.clone() } };
                        if !section.text.is_empty() && !section.text.ends_with("\n\n") { section.text.push('\n'); }
                        section.text.push_str(&format!("[Tabla T{:02} → tablas/T{:02}.csv]\n\n", id + 1, id + 1));
                        section.tables.push(id);
                        doc.tables.push(Table { title, rows: std::mem::take(&mut rows), colors: std::mem::take(&mut table_colors), ..Default::default() });
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

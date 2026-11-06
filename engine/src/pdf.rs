//! PDF: text page by page. Sections come from the document's own bookmarks
//! (outline) when it has them, levels 1-2, each covering the pages up to the
//! next one; without bookmarks every page is a section. Page markers stay in
//! the text so any line can be cited by page. No OCR: a scanned PDF is said
//! to be one and its page images are the way to read it.
use crate::{Document, Section};

/// Below this many characters per page on average, the pages are images.
const SCANNED_CHARS_PER_PAGE: usize = 40;

pub fn extract(bytes: &[u8]) -> Result<Document, String> {
    let pages = pdf_extract::extract_text_from_mem_by_pages(bytes).map_err(|e| format!("PDF ilegible: {e}"))?;
    let count = pages.len() as u32;
    let mut doc = Document { kind: "pdf", pages: Some(count), ..Default::default() };
    if count == 0 { return Err("PDF sin páginas".into()) }
    let chars: usize = pages.iter().map(|p| p.trim().chars().count()).sum();
    if chars < SCANNED_CHARS_PER_PAGE * pages.len() {
        doc.notes.push("El PDF casi no tiene texto (probablemente escaneado). DocPack no hace OCR: lee las imágenes de paginas/.".into());
    }
    doc.notes.push("Las tablas de un PDF no se detectan todavía: aparecen como texto dentro de su sección.".into());
    let text_of = |from: u32, to: u32| -> String {
        (from..=to).map(|p| format!("<!-- página {p} -->\n{}\n", pages[(p - 1) as usize].trim())).collect::<Vec<_>>().join("\n")
    };
    // Bookmarks: (page, title), levels 1-2, merged when several start on one page.
    let mut marks: Vec<(u32, String, u8)> = Vec::new();
    if let Ok(pdf) = lopdf::Document::load_mem(bytes) {
        if let Ok(toc) = pdf.get_toc() {
            for item in toc.toc.iter().filter(|t| t.level <= 2 && t.page >= 1 && t.page as u32 <= count) {
                let title = item.title.trim().to_string();
                if title.is_empty() { continue }
                match marks.last_mut() {
                    Some(last) if last.0 == item.page as u32 => { last.1 = format!("{} · {}", last.1, title) }
                    _ => marks.push((item.page as u32, title, item.level as u8)),
                }
            }
        }
    }
    marks.sort_by_key(|m| m.0);
    marks.dedup_by_key(|m| m.0);
    if marks.is_empty() {
        doc.notes.push("El PDF no tiene marcadores: una sección por página.".into());
        for p in 1..=count {
            let first = pages[(p - 1) as usize].lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(70).collect::<String>();
            let title = if first.is_empty() { format!("Página {p}") } else { format!("Página {p}: {first}") };
            doc.sections.push(Section { title, level: 1, page_start: Some(p), page_end: Some(p), text: text_of(p, p), ..Default::default() });
        }
        return Ok(doc);
    }
    if marks[0].0 > 1 {
        doc.sections.push(Section { title: "Inicio".into(), level: 1, page_start: Some(1), page_end: Some(marks[0].0 - 1), text: text_of(1, marks[0].0 - 1), ..Default::default() });
    }
    for (i, (page, title, level)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map(|m| m.0.saturating_sub(1).max(*page)).unwrap_or(count);
        doc.sections.push(Section { title: title.clone(), level: (*level).max(1), page_start: Some(*page), page_end: Some(end), text: text_of(*page, end), ..Default::default() });
    }
    Ok(doc)
}

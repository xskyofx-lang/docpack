//! DocPack: turns a document into a folder an AI reads by parts.
//!
//! `<project>/docpack/<file name>/` holds `manifest.json` (source, SHA-256,
//! extractor), `index.json` (sections with pages and a token estimate, tables,
//! page images), `secciones/*.md`, `tablas/*.csv` and, for a PDF on Windows,
//! `paginas/*.png`. The original is only read, never changed. A pack whose
//! manifest has the same source SHA and DocPack version is left as it is.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

mod docx;
mod ooxml;
mod paginate;
mod pdf;
mod pptx;
mod render;
mod xlsx;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Formats DocPack reads.
pub const FORMATS: &[&str] = &["pdf", "docx", "docm", "xlsx", "xlsm", "xls", "xlsb", "ods", "pptx"];
/// Larger files are refused: a document is read whole into memory.
pub const MAX_SOURCE_BYTES: u64 = 200 * 1024 * 1024;

/// One part of the document, in reading order.
#[derive(Debug, Clone, Default)]
pub struct Section {
    pub title: String,
    /// 0 for the whole-document part before the first heading, 1.. for headings.
    pub level: u8,
    pub page_start: Option<u32>,
    pub page_end: Option<u32>,
    /// Markdown text; tables are referenced, not repeated.
    pub text: String,
    pub tables: Vec<usize>,
    pub figures: Vec<usize>,
    /// Titles of the headings above this one that had no content of their own.
    pub path: Vec<String>,
    /// Text repeated on every page (headers and footers): no page of its own.
    pub every_page: bool,
}

/// A table, written as CSV once.
#[derive(Debug, Clone, Default)]
pub struct Table {
    pub title: String,
    pub page: Option<u32>,
    pub rows: Vec<Vec<String>>,
}

/// An image embedded in the document (a diagram, a screenshot), kept as the
/// document stores it.
#[derive(Debug, Clone, Default)]
pub struct Figure {
    pub section: String,
    pub extension: String,
    pub bytes: Vec<u8>,
}

/// What an extractor returns.
#[derive(Debug, Default)]
pub struct Document {
    pub kind: &'static str,
    pub pages: Option<u32>,
    pub sections: Vec<Section>,
    pub tables: Vec<Table>,
    pub figures: Vec<Figure>,
    /// Limits of this extraction, stated in the manifest (never hidden).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Page numbers and page images: render PDF pages to PNG and lay a Word
    /// document out with an engine of this machine to know its pages.
    pub pages: bool,
    /// Rebuild even when the pack is current.
    pub force: bool,
}

/// The outcome of a build, as the CLI and the hook report it.
#[derive(Debug, Serialize)]
pub struct Built {
    pub pack: PathBuf,
    pub index: PathBuf,
    pub status: &'static str,
    pub sections: usize,
    pub tables: usize,
    pub page_images: usize,
    pub tokens_estimate: u64,
}

pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// A token count for planning what to read, never a measurement. Calibrated
/// on a Spanish technical design (tables, codes, field names) whose tokens
/// were measured: 40 508 for 1 000 lines, where "four characters a token" said
/// about 20 000. A word costs one token plus one per ~2.2 further letters;
/// digits go two a token; every other symbol and every line break is one.
/// Plain English prose comes out a little high, which is the safe side.
pub fn tokens_estimate(text: &str) -> u64 {
    let (mut tokens, mut letters, mut digits) = (0f64, 0usize, 0usize);
    let flush = |tokens: &mut f64, letters: &mut usize, digits: &mut usize| {
        if *letters > 0 { *tokens += 1.0 + (*letters - 1) as f64 / 2.2; *letters = 0; }
        if *digits > 0 { *tokens += (*digits as f64 / 2.0).ceil(); *digits = 0; }
    };
    for c in text.chars() {
        if c.is_alphabetic() { if digits > 0 { flush(&mut tokens, &mut letters, &mut digits) } letters += 1 }
        else if c.is_ascii_digit() { if letters > 0 { flush(&mut tokens, &mut letters, &mut digits) } digits += 1 }
        else {
            flush(&mut tokens, &mut letters, &mut digits);
            if c == '\n' || !c.is_whitespace() { tokens += 1.0 }
        }
    }
    flush(&mut tokens, &mut letters, &mut digits);
    tokens.ceil() as u64
}

/// Where a long section can be entered: the list items that have items under
/// them (a group of fields, a group of steps), with their line in the file, so
/// it can be read in parts with offset/limit. Only the document's own list
/// structure is used; nothing is guessed from words or formatting.
fn outline(body: &str) -> Vec<serde_json::Value> {
    let lines: Vec<&str> = body.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(item) = line.strip_prefix("- ") else { continue };
        let child = lines.get(i + 1).is_some_and(|next| next.starts_with("  - "));
        if child { out.push(serde_json::json!({"line": i + 1, "title": item.chars().take(70).collect::<String>()})); }
    }
    out
}

/// About what one part of a long section holds: small enough to open only the
/// part a question needs, large enough that a section is not a list of crumbs.
const PART_TOKENS: u64 = 2_000;

/// A long section in parts cut between paragraphs (a blank line), each with
/// its lines and its first line as a hint. Mechanical, so it works whatever
/// the document's formatting; nothing is guessed about where a topic starts.
fn parts(body: &str) -> Vec<serde_json::Value> {
    if tokens_estimate(body) <= PART_TOKENS * 2 { return Vec::new() }
    let lines: Vec<&str> = body.lines().collect();
    let mut out = Vec::new();
    let (mut start, mut acc) = (0usize, 0u64);
    for (i, line) in lines.iter().enumerate() {
        acc += tokens_estimate(line) + 1;
        let last = i + 1 == lines.len();
        if (acc >= PART_TOKENS && line.trim().is_empty()) || last {
            let hint = lines[start..=i].iter().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("").chars().take(60).collect::<String>();
            out.push(serde_json::json!({"lines": [start + 1, i + 1], "tokens_estimate": acc, "starts": hint}));
            start = i + 1;
            acc = 0;
        }
    }
    out
}

/// Drops null and empty fields: every one is tokens in the index.
fn compact(mut v: serde_json::Value) -> serde_json::Value {
    if let Some(map) = v.as_object_mut() {
        map.retain(|_, x| !(x.is_null() || x.as_array().is_some_and(|a| a.is_empty()) || x.as_str() == Some("")));
    }
    v
}

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub fn supported(path: &Path) -> bool {
    FORMATS.contains(&extension(path).as_str())
}

/// The folder of a document's pack: `<project>/docpack/<file name>/`. A file
/// outside the project gets its pack next to itself.
pub fn pack_dir(project: &Path, source: &Path) -> PathBuf {
    let inside = match (project.canonicalize(), source.canonicalize()) {
        (Ok(p), Ok(s)) => s.starts_with(&p),
        _ => false,
    };
    let base = if inside { project.to_path_buf() } else { source.parent().unwrap_or(Path::new(".")).to_path_buf() };
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "documento".into());
    base.join("docpack").join(safe_name(&name))
}

fn safe_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() { "documento".into() } else { cleaned }
}

fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        let c = match c { 'á' | 'à' | 'ä' | 'â' => 'a', 'é' | 'è' | 'ë' | 'ê' => 'e', 'í' | 'ì' | 'ï' | 'î' => 'i', 'ó' | 'ò' | 'ö' | 'ô' => 'o', 'ú' | 'ù' | 'ü' | 'û' => 'u', 'ñ' => 'n', c => c };
        if c.is_ascii_alphanumeric() { out.push(c) } else if !out.ends_with('-') && !out.is_empty() { out.push('-') }
        if out.len() >= 40 { break }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "seccion".into() } else { out }
}

pub fn extract(bytes: &[u8], ext: &str) -> Result<Document, String> {
    match ext {
        "pdf" => pdf::extract(bytes),
        "docx" | "docm" => docx::extract(bytes),
        "pptx" => pptx::extract(bytes),
        "xlsx" | "xlsm" | "xls" | "xlsb" | "ods" => xlsx::extract(bytes, ext),
        other => Err(format!("Formato no compatible: .{other}")),
    }
}

/// The pack's manifest when it is current for these bytes.
fn current(dir: &Path, sha: &str) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
    (manifest["source"]["sha256"] == sha && manifest["docpack_version"] == VERSION).then_some(manifest)
}

fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) { format!("\"{}\"", value.replace('"', "\"\"")) } else { value.to_string() }
}

/// Builds (or reuses) the pack of `source` under `project`.
pub fn build(source: &Path, project: &Path, options: &Options) -> Result<Built, String> {
    let meta = std::fs::metadata(source).map_err(|e| format!("No se pudo leer {}: {e}", source.display()))?;
    if !meta.is_file() { return Err(format!("{} no es un archivo", source.display())) }
    if meta.len() > MAX_SOURCE_BYTES { return Err(format!("{} supera {} MB", source.display(), MAX_SOURCE_BYTES / 1024 / 1024)) }
    let ext = extension(source);
    if !FORMATS.contains(&ext.as_str()) { return Err(format!("Formato no compatible: .{ext}")) }
    let bytes = std::fs::read(source).map_err(|e| format!("No se pudo leer {}: {e}", source.display()))?;
    let sha = sha256(&bytes);
    let dir = pack_dir(project, source);
    if !options.force {
        if let Some(m) = current(&dir, &sha) {
            return Ok(Built {
                index: dir.join("index.json"), pack: dir, status: "up_to_date",
                sections: m["stats"]["sections"].as_u64().unwrap_or(0) as usize,
                tables: m["stats"]["tables"].as_u64().unwrap_or(0) as usize,
                page_images: m["stats"]["page_images"].as_u64().unwrap_or(0) as usize,
                tokens_estimate: m["stats"]["tokens_total_estimate"].as_u64().unwrap_or(0),
            });
        }
    }
    let mut doc = extract(&bytes, &ext)?;
    // The PDF whose pages are the document's pages: the file itself, or Word
    // laid out by an engine of this machine (paginate.rs).
    let mut laid: Option<Vec<u8>> = None;
    if matches!(ext.as_str(), "docx" | "docm") && options.pages {
        match paginate::layout(&bytes, &ext) {
            Ok(l) => { paginate::assign_pages(&mut doc, &l); laid = Some(l.pdf) }
            Err(e) => {
                doc.notes.retain(|n| !n.starts_with("Word no guarda páginas"));
                doc.notes.push(format!("Sin números de página: Word no guarda páginas y ningún motor de este equipo pudo paginarlo ({e}). Las referencias son por sección y línea."));
            }
        }
    }
    fold_empty_headings(&mut doc);
    let pages_pdf = if ext == "pdf" { Some(bytes.as_slice()) } else { laid.as_deref() };
    write_pack(&dir, source, &bytes, &sha, doc, options, pages_pdf)
}

/// A heading with nothing under it but another heading (a title, then a
/// chapter, then its first section) is not a section to open: its title goes to
/// the next section's path, so the index has no empty files.
fn fold_empty_headings(doc: &mut Document) {
    let mut out: Vec<Section> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for mut s in std::mem::take(&mut doc.sections) {
        if s.text.trim().is_empty() && s.tables.is_empty() && s.figures.is_empty() {
            let title = s.title.trim().to_string();
            if pending.last() != Some(&title) { pending.push(title); }
            continue;
        }
        let mut path = std::mem::take(&mut pending);
        path.dedup();
        if path.last().map(String::as_str) == Some(s.title.trim()) { path.pop(); }
        s.path = path;
        out.push(s);
    }
    // Headings at the very end with nothing under them are kept as one section
    // so no title of the document is lost.
    if !pending.is_empty() { out.push(Section { title: pending.join(" › "), level: 1, text: "(Sin contenido debajo de este título.)".into(), ..Default::default() }); }
    doc.sections = out;
}

fn write_pack(dir: &Path, source: &Path, bytes: &[u8], sha: &str, mut doc: Document, options: &Options, pages_pdf: Option<&[u8]>) -> Result<Built, String> {
    let io = |e: std::io::Error| e.to_string();
    let parent = dir.parent().ok_or("Carpeta de pack inválida")?;
    std::fs::create_dir_all(parent).map_err(io)?;
    // Written aside and swapped in, so a reader never sees half a pack.
    let staging = parent.join(format!(".{}.tmp-{}", dir.file_name().and_then(|n| n.to_str()).unwrap_or("pack"), std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    for sub in ["secciones", "tablas"] { std::fs::create_dir_all(staging.join(sub)).map_err(io)?; }
    let mut figure_entries = Vec::new();
    if !doc.figures.is_empty() { std::fs::create_dir_all(staging.join("figuras")).map_err(io)?; }
    for (i, figure) in doc.figures.iter().enumerate() {
        let id = format!("F{:02}", i + 1);
        let file = format!("figuras/{id}.{}", figure.extension);
        std::fs::write(staging.join(&file), &figure.bytes).map_err(io)?;
        // A vision model reads raster images; EMF/WMF (Visio, Office drawings) it cannot.
        let readable = matches!(figure.extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp");
        figure_entries.push(serde_json::json!({"id": id, "section": figure.section, "file": file, "bytes": figure.bytes.len(), "image_readable": readable}));
    }

    let mut table_entries = Vec::new();
    // Each table names the section it belongs to (its id), not that section's
    // title again: with 89 tables the repeated titles were most of the index.
    let owner: std::collections::HashMap<usize, String> = doc.sections.iter().enumerate().flat_map(|(n, s)| s.tables.iter().map(move |t| (*t, format!("S{:02}", n + 1)))).collect();
    for (i, table) in doc.tables.iter().enumerate() {
        let id = format!("T{:02}", i + 1);
        let file = format!("tablas/{id}.csv");
        let csv: String = table.rows.iter().map(|r| r.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join("\n");
        std::fs::write(staging.join(&file), format!("\u{feff}{csv}\n")).map_err(io)?;
        let columns = table.rows.iter().map(Vec::len).max().unwrap_or(0);
        table_entries.push(serde_json::json!({"id": id, "section": owner.get(&i).cloned(), "page": table.page, "rows": table.rows.len(), "columns": columns, "file": file, "tokens_estimate": tokens_estimate(&csv)}));
    }
    let mut section_entries = Vec::new();
    let mut total = 0u64;
    for (i, section) in doc.sections.iter_mut().enumerate() {
        let id = format!("S{:02}", i + 1);
        let file = format!("secciones/{:02}-{}.md", i + 1, slug(&section.title));
        let refs: Vec<String> = section.tables.iter().map(|t| format!("T{:02}", t + 1)).collect();
        let figs: Vec<String> = section.figures.iter().map(|f| format!("F{:02}", f + 1)).collect();
        let mut body = String::new();
        if !section.path.is_empty() { body.push_str(&format!("_{}_\n\n", section.path.join(" › "))); }
        body.push_str(&format!("# {}\n\n", section.title.trim()));
        if let (Some(a), Some(b)) = (section.page_start, section.page_end) {
            body.push_str(&if a == b { format!("_Página {a}_\n\n") } else { format!("_Páginas {a}–{b}_\n\n") });
        }
        body.push_str(section.text.trim());
        body.push('\n');
        let tokens = tokens_estimate(&body);
        total += tokens;
        std::fs::write(staging.join(&file), &body).map_err(io)?;
        let lines = body.lines().count();
        section_entries.push(compact(serde_json::json!({"id": id, "title": section.title.trim(), "level": section.level, "path": section.path, "page_start": section.page_start, "page_end": section.page_end, "file": file, "lines": lines, "tables": refs, "figures": figs, "tokens_estimate": tokens, "outline": outline(&body), "parts": parts(&body)})));
    }
    total += table_entries.iter().map(|t| t["tokens_estimate"].as_u64().unwrap_or(0)).sum::<u64>();

    let mut page_images = Vec::new();
    if let (Some(pdf), true) = (pages_pdf, options.pages) {
        match render::pdf_pages(pdf, &staging.join("paginas")) {
            Ok(files) => page_images = files,
            Err(e) => doc.notes.push(format!("Imágenes de página no generadas: {e}")),
        }
    }
    let file_name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let index = serde_json::json!({
        "document": file_name, "kind": doc.kind, "pages": doc.pages,
        "how_to_read": "Lee este índice; abre solo las secciones que necesites y, de ellas, sus tablas (tablas/Tnn.csv) y figuras. Una sección larga trae outline con líneas para leerla por partes. Detalle de tablas en tablas/index.json y de figuras en figuras/index.json. paginas/pNNN.png solo para ver una página. El original solo si la persona lo pide.",
        "sections": section_entries,
        "tables": table_entries.len(), "figures": figure_entries.len(),
        "page_images": if page_images.is_empty() { serde_json::Value::Null } else { serde_json::json!({"count": page_images.len(), "file": "paginas/pNNN.png"}) },
    });
    let index = compact(index);
    std::fs::write(staging.join("tablas").join("index.json"), serde_json::to_string(&table_entries).map_err(|e| e.to_string())?).map_err(io)?;
    if !figure_entries.is_empty() { std::fs::write(staging.join("figuras").join("index.json"), serde_json::to_string(&figure_entries).map_err(|e| e.to_string())?).map_err(io)?; }
    let manifest = serde_json::json!({
        "docpack_version": VERSION,
        "source": {"file": source.to_string_lossy(), "name": file_name, "sha256": sha, "bytes": bytes.len(), "kind": doc.kind, "pages": doc.pages},
        "extractor": {"name": "docpack", "version": VERSION, "ocr": false},
        "stats": {"sections": section_entries.len(), "tables": table_entries.len(), "figures": figure_entries.len(), "page_images": page_images.len(), "tokens_total_estimate": total},
        "notes": doc.notes,
        "original_unchanged": true,
    });
    // The index is what the AI reads first: compact JSON, no indentation tokens.
    std::fs::write(staging.join("index.json"), serde_json::to_string(&index).map_err(|e| e.to_string())?).map_err(io)?;
    std::fs::write(staging.join("manifest.json"), serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(io)?;

    // The previous pack of this file is replaced only when it is a DocPack pack.
    if dir.exists() {
        if !dir.join("manifest.json").is_file() {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("{} existe y no es un pack de DocPack; no se sobrescribe", dir.display()));
        }
        std::fs::remove_dir_all(dir).map_err(io)?;
    }
    std::fs::rename(&staging, dir).map_err(io)?;
    Ok(Built { index: dir.join("index.json"), pack: dir.to_path_buf(), status: "built", sections: section_entries.len(), tables: table_entries.len(), page_images: page_images.len(), tokens_estimate: total })
}

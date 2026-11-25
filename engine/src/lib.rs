//! DocPack: turns a document into a folder an AI reads by parts.
//!
//! `<project>/docpack/<file name>/` holds `manifest.json` (source, SHA-256,
//! extractor), `index.json` (sections with pages and a token estimate, tables,
//! page images), `secciones/*.md`, `tablas/*.csv` and `paginas/*.png`. The
//! original is only read, never changed. A pack whose manifest has the same
//! source SHA and DocPack version is left as it is. `docpack/index.json` lists
//! every pack of the folder (library.rs).
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

mod about;
pub mod color;
mod docx;
pub mod library;
mod ooxml;
mod paginate;
mod pdf;
mod pptx;
mod render;
mod xlsx;
mod xlsx_color;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How to audit many documents with DocPack (guide/audit.md): what an agent
/// that did not design DocPack needs to use it well. `docpack guide` and the
/// MCP tool `docpack_guide` give it.
pub const AUDIT_GUIDE: &str = include_str!("../guide/audit.md");
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
    /// A table of contents: its lines end in a page number after dot leaders
    /// or a tab. Not where an answer is.
    pub contents: bool,
}

/// A table, written as CSV once.
#[derive(Debug, Clone, Default)]
pub struct Table {
    /// Its caption or the heading above it; else the title of its section.
    pub title: String,
    /// The page it starts on.
    pub page: Option<u32>,
    /// Every page it covers, when it goes on across pages.
    pub pages: Vec<u32>,
    pub rows: Vec<Vec<String>>,
    /// Why its cells must be checked with the image of its page before they
    /// are quoted (it holds another table's caption); empty when nothing is
    /// known against it.
    pub check: String,
    /// The cells the document marks with a color: their fill, and the pieces
    /// of their text in a color of their own. The CSV keeps the plain text.
    pub colors: Vec<CellColor>,
    /// Spreadsheet cells calculated by a formula: (row, column as in the
    /// CSV, the formula with its `=`). The CSV keeps the value shown.
    pub formulas: Vec<(usize, usize, String)>,
    /// The row of the CSV that holds the column names: 0, or the row below
    /// the title lines a sheet carries above its table.
    pub header_row: usize,
    /// A spreadsheet's table: its sheet, the sheet's cell the CSV starts at
    /// (row, column from 0), and whether the sheet is hidden or protected.
    pub sheet: Option<Sheet>,
    /// What a review of its cells should look at (a spreadsheet's errors and
    /// numbers typed among formulas), each at its place in the CSV.
    pub findings: Vec<Finding>,
}

/// The sheet a spreadsheet's table comes from.
#[derive(Debug, Clone, Default)]
pub struct Sheet { pub name: String, pub origin: (u32, u32), pub hidden: bool, pub protected: bool }

/// A cell a review should look at: what it is (`error`, `constante`), its
/// row and column in the CSV, the value it shows and why.
#[derive(Debug, Clone, Default)]
pub struct Finding { pub kind: &'static str, pub row: usize, pub column: usize, pub value: String, pub detail: String }

/// A piece of text in a color (or on a highlight) that marks it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mark { pub text: String, pub color: Option<String>, pub fill: Option<String> }

/// A table cell the document marks with color: row and column as in its CSV.
#[derive(Debug, Clone, Default)]
pub struct CellColor { pub row: usize, pub column: usize, pub fill: Option<String>, pub text: Vec<Mark> }

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
    /// The OCR engine that read text from page images, when one did.
    pub ocr: Option<String>,
    /// Pages whose image the pack carries even when it carries no page
    /// images: text that looks like a table and stayed as text.
    pub check_pages: Vec<u32>,
    /// What the document says of itself: title, version and date.
    pub about: About,
    /// The colors the document marks with, how often and on what text.
    pub colors: color::Legend,
    /// The number printed on a page when it is not its place in the file
    /// ("9" on the PDF's page 11): what the document is quoted by.
    pub printed: std::collections::BTreeMap<u32, String>,
    /// Pages whose text may not be the whole page, and why: `ocr` (read by
    /// OCR), `sin_texto` (no readable text), `texto_dañado` (letters the PDF
    /// cannot convert), `imagen_sin_ocr` (a large picture OCR could not read).
    /// A search that finds nothing says so.
    pub doubtful_pages: std::collections::BTreeMap<u32, &'static str>,
}

/// A document's own title, version and date (its properties; the version
/// also from a "v1.4" in its title or file name).
#[derive(Debug, Clone, Default, Serialize)]
pub struct About {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Page images: every page of a PDF (or of a Word document as laid out)
    /// to PNG. Without them only the pages to check carry their image. A Word
    /// document is laid out to know its pages either way, when this machine
    /// has an engine for it.
    pub pages: bool,
    /// Rebuild even when the pack is current.
    pub force: bool,
    /// The project was named on purpose (`--project`): the pack of a file
    /// outside it goes in the project too, never next to the file.
    pub in_project: bool,
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
/// its lines and its first line as a hint. Where the document marks its own
/// subheadings (a Markdown heading inside the section), a part also ends
/// before one once it holds half a part, and its hint is the first
/// heading it holds: a part is then a field or a topic, not a slice.
fn parts(body: &str) -> Vec<serde_json::Value> {
    if tokens_estimate(body) <= PART_TOKENS * 2 { return Vec::new() }
    let lines: Vec<&str> = body.lines().collect();
    let heading = |l: &str| l.starts_with("## ") || l.starts_with("### ") || l.starts_with("#### ") || l.starts_with("##### ") || l.starts_with("###### ");
    let mut out = Vec::new();
    let (mut start, mut acc) = (0usize, 0u64);
    for (i, line) in lines.iter().enumerate() {
        acc += tokens_estimate(line) + 1;
        let last = i + 1 == lines.len();
        let before_heading = acc >= PART_TOKENS / 2 && lines.get(i + 1).is_some_and(|next| heading(next));
        if (acc >= PART_TOKENS && line.trim().is_empty()) || before_heading || last {
            let part = &lines[start..=i];
            let hint = part.iter().find(|l| heading(l)).map(|l| l.trim_start_matches('#').trim())
                .or_else(|| part.iter().map(|l| l.trim()).find(|l| !l.is_empty()))
                .unwrap_or("").chars().take(60).collect::<String>();
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
    pack_dir_with(project, source, &Options::default())
}

/// [`pack_dir`] as a build with these options places it: with
/// `in_project`, a file outside the project gets its pack in the project too.
pub fn pack_dir_with(project: &Path, source: &Path, options: &Options) -> PathBuf {
    let inside = options.in_project || match (project.canonicalize(), source.canonicalize()) {
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

/// The text of a caught panic's payload.
#[doc(hidden)]
pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload.downcast_ref::<&str>().map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "pánico sin mensaje".into())
}

pub fn extract(bytes: &[u8], ext: &str) -> Result<Document, String> {
    // A reader that panics on a strange file becomes an error for that file:
    // a build of many documents (or the MCP process) goes on with the rest.
    let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match ext {
        "pdf" => pdf::extract(bytes),
        "docx" | "docm" => docx::extract(bytes),
        "pptx" => pptx::extract(bytes),
        "xlsx" | "xlsm" | "xls" | "xlsb" | "ods" => xlsx::extract(bytes, ext),
        other => Err(format!("Formato no compatible: .{other}")),
    }));
    let mut doc = match read {
        Ok(result) => result?,
        Err(payload) => return Err(format!("El lector de .{ext} falló al leer el archivo (etapa: extracción): {}", panic_message(payload.as_ref()))),
    };
    if ext != "pdf" { doc.about = about::office(bytes) }
    for section in &mut doc.sections { section.contents = about::is_contents(&section.text) }
    Ok(doc)
}

/// The pack's manifest when it is current for these bytes and has what is
/// asked: a pack built without pages (a quick preview) is rebuilt when pages
/// are wanted, so they are never missing for good.
fn current(dir: &Path, sha: &str, options: &Options) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
    let has_pages = manifest["options"]["pages"].as_bool().unwrap_or(false);
    (manifest["source"]["sha256"] == sha && manifest["docpack_version"] == VERSION && (has_pages || !options.pages)).then_some(manifest)
}

pub(crate) fn csv_cell(value: &str) -> String {
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
    let dir = pack_dir_with(project, source, options);
    if !options.force {
        if let Some(m) = current(&dir, &sha, options) {
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
    if matches!(ext.as_str(), "docx" | "docm") {
        match paginate::layout(&bytes, &ext) {
            Ok(l) => { paginate::assign_pages(&mut doc, &l); laid = Some(l.pdf) }
            Err(e) => {
                doc.notes.retain(|n| !n.starts_with("Word no guarda páginas"));
                doc.notes.push(format!("Sin números de página: Word no guarda páginas y ningún motor de este equipo pudo paginarlo ({e}). Las referencias son por sección y línea."));
            }
        }
    }
    fold_empty_headings(&mut doc);
    let file_name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    about::complete(&mut doc.about, &file_name);
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

/// An I/O step that Windows (or an antivirus, the indexer, another process
/// holding a handle) can refuse for a moment: "Acceso denegado (os error 5)",
/// a sharing violation (32) or a lock violation (33). It is tried again after
/// short waits; when it still fails, the error names the operation and the
/// paths involved, which a bare OS error does not.
pub(crate) fn retry_io<T>(op: &str, paths: &[&Path], f: impl FnMut() -> std::io::Result<T>) -> Result<T, String> {
    retry_io_with(&[25, 50, 100, 200, 400, 800], op, paths, f)
}

pub(crate) fn retry_io_with<T>(waits_ms: &[u64], op: &str, paths: &[&Path], mut f: impl FnMut() -> std::io::Result<T>) -> Result<T, String> {
    let mut attempt = 0;
    loop {
        match f() {
            Ok(value) => return Ok(value),
            Err(e) => {
                let busy = e.kind() == std::io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33));
                if busy && attempt < waits_ms.len() {
                    std::thread::sleep(std::time::Duration::from_millis(waits_ms[attempt]));
                    attempt += 1;
                    continue;
                }
                let shown = paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" → ");
                let tries = if attempt > 0 { format!(" (tras {} intentos)", attempt + 1) } else { String::new() };
                return Err(format!("{op} {shown}: {e}{tries}"));
            }
        }
    }
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
        // Its header (the first row, or the row below a sheet's title lines)
        // and every page it covers when more than one.
        let header: Vec<String> = table.rows.get(table.header_row).map(|r| r.iter().map(|c| c.split_whitespace().collect::<Vec<_>>().join(" ")).collect()).unwrap_or_default();
        let header_row = (table.header_row > 0).then_some(table.header_row);
        let pages = (table.pages.len() > 1).then_some(&table.pages);
        let tokens = tokens_estimate(&csv);
        // A table larger than a section's part is data: filtered, not read.
        // A spreadsheet's cell as its sheet names it ("D14"), from its place
        // in the CSV.
        let cell = |r: usize, c: usize| table.sheet.as_ref().map(|s| library::cell_name(s.origin.0 + r as u32, s.origin.1 + c as u32));
        let query = (tokens > PART_TOKENS * 10).then_some("Tabla de datos grande: no la leas entera; fíltrala o resúmela con un script sobre el CSV (encabezado en header).");
        // Its colors beside it: which cells, which fill, which words.
        let (colors_file, colors) = if table.colors.is_empty() { (None, serde_json::Value::Null) } else {
            let path = format!("tablas/{id}.colores.json");
            let cells: Vec<serde_json::Value> = table.colors.iter().map(|c| compact(serde_json::json!({
                "fila": c.row, "columna": c.column, "celda": cell(c.row, c.column), "fondo": c.fill.as_deref().map(color::label),
                "texto": c.text.iter().map(|m| compact(serde_json::json!({"texto": m.text, "color": m.color.as_deref().map(color::label), "resaltado": m.fill.as_deref().map(color::label)}))).collect::<Vec<_>>(),
            }))).collect();
            std::fs::write(staging.join(&path), serde_json::to_string(&cells).map_err(|e| e.to_string())?).map_err(io)?;
            let mut count: std::collections::BTreeMap<String, usize> = Default::default();
            for c in &table.colors {
                if let Some(f) = &c.fill { *count.entry(format!("fondo {}", color::label(f))).or_default() += 1 }
                for m in &c.text {
                    if let Some(k) = &m.color { *count.entry(format!("texto {}", color::label(k))).or_default() += 1 }
                    if let Some(k) = &m.fill { *count.entry(format!("resaltado {}", color::label(k))).or_default() += 1 }
                }
            }
            (Some(path), serde_json::json!(count))
        };
        // Its formulas beside it: the CSV keeps what the cell shows, this says
        // how it is calculated.
        let (formulas_file, formulas) = if table.formulas.is_empty() { (None, None) } else {
            let path = format!("tablas/{id}.formulas.json");
            let cells: Vec<serde_json::Value> = table.formulas.iter().map(|(row, column, formula)| compact(serde_json::json!({"fila": row, "columna": column, "celda": cell(*row, *column), "formula": formula}))).collect();
            std::fs::write(staging.join(&path), serde_json::to_string(&cells).map_err(|e| e.to_string())?).map_err(io)?;
            (Some(path), Some(table.formulas.len()))
        };
        // What a review of its cells should look at, beside it.
        let (review_file, review) = if table.findings.is_empty() { (None, None) } else {
            let path = format!("tablas/{id}.revision.json");
            let cells: Vec<serde_json::Value> = table.findings.iter().map(|f| compact(serde_json::json!({"tipo": f.kind, "fila": f.row, "columna": f.column, "celda": cell(f.row, f.column), "valor": f.value, "detalle": f.detail}))).collect();
            std::fs::write(staging.join(&path), serde_json::to_string(&cells).map_err(|e| e.to_string())?).map_err(io)?;
            let mut count: std::collections::BTreeMap<&str, usize> = Default::default();
            for f in &table.findings { *count.entry(f.kind).or_default() += 1 }
            (Some(path), Some(count))
        };
        let sheet = table.sheet.as_ref();
        table_entries.push(compact(serde_json::json!({"id": id, "sheet": sheet.map(|s| &s.name), "origin": cell(0, 0), "hidden": sheet.is_some_and(|s| s.hidden).then_some(true), "protected": sheet.is_some_and(|s| s.protected).then_some(true), "review": review, "review_file": review_file, "title": table.title.trim(), "section": owner.get(&i).cloned(), "page": table.page, "printed_page": table.page.and_then(|p| doc.printed.get(&p)), "pages": pages, "rows": table.rows.len(), "columns": columns, "header": header, "header_row": header_row, "file": file, "tokens_estimate": tokens, "read": query, "check": table.check, "colores": colors, "colores_file": colors_file, "formulas": formulas, "formulas_file": formulas_file})));
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
        // Its printed numbers beside the PDF's, when the document numbers its
        // pages apart.
        let printed_start = section.page_start.and_then(|p| doc.printed.get(&p));
        let printed_end = section.page_end.and_then(|p| doc.printed.get(&p));
        if let (Some(a), Some(b)) = (section.page_start, section.page_end) {
            let printed = match (printed_start, printed_end) {
                (Some(x), Some(y)) if x != y => format!(" · impresas {x}–{y}"),
                (Some(x), Some(_)) => format!(" · impresa {x}"),
                (Some(x), None) => format!(" · impresa desde {x}"),
                (None, Some(y)) => format!(" · impresa hasta {y}"),
                (None, None) => String::new(),
            };
            body.push_str(&if a == b { format!("_Página {a}{printed}_\n\n") } else { format!("_Páginas {a}–{b}{printed}_\n\n") });
        }
        body.push_str(section.text.trim());
        body.push('\n');
        let tokens = tokens_estimate(&body);
        total += tokens;
        std::fs::write(staging.join(&file), &body).map_err(io)?;
        let lines = body.lines().count();
        section_entries.push(compact(serde_json::json!({"id": id, "title": section.title.trim(), "kind": section.contents.then_some("toc"), "level": section.level, "path": section.path, "page_start": section.page_start, "page_end": section.page_end, "printed_start": printed_start, "printed_end": printed_end, "file": file, "lines": lines, "tables": refs, "figures": figs, "tokens_estimate": tokens, "outline": outline(&body), "parts": parts(&body)})));
    }
    total += table_entries.iter().map(|t| t["tokens_estimate"].as_u64().unwrap_or(0)).sum::<u64>();

    // Every page when the pack carries page images; else only the pages to
    // check with their image (text that looks like a table).
    let mut page_images = Vec::new();
    let only = (!options.pages).then_some(doc.check_pages.as_slice());
    if let Some(pdf) = pages_pdf.filter(|_| options.pages || !doc.check_pages.is_empty()) {
        match render::pdf_pages(pdf, &staging.join("paginas"), only) {
            Ok(files) => page_images = files,
            Err(e) => doc.notes.push(format!("Imágenes de página no generadas: {e}")),
        }
    }
    // The colors the document marks with: a legend to tell "the pink ones"
    // from the purple ones.
    let legend: Vec<serde_json::Value> = doc.colors.entries().iter().map(|(kind, hex, u)| serde_json::json!({"uso": kind, "color": hex, "nombre": color::name(hex), "veces": u.count, "ejemplos": u.samples})).collect();
    if !legend.is_empty() {
        let top: Vec<String> = doc.colors.entries().iter().take(6).map(|(kind, hex, u)| format!("{kind} {} {hex} ({} {})", color::name(hex), u.count, if u.count == 1 { "vez" } else { "veces" })).collect();
        doc.notes.push(format!("El documento marca con colores: {}{}. El color de cada celda está en tablas/Tnn.colores.json (el CSV lleva el texto sin color) y el texto marcado de las secciones va entre <span style=\"color:#RRGGBB\"> o <mark style=\"background:#RRGGBB\">; la lista completa, en colores de este manifiesto.", top.join(", "), if legend.len() > 6 { " y más" } else { "" }));
    }
    // A long document's detail of each section (its parts, outline, tables
    // and figures) costs more to read than the parts it points to: it goes to
    // secciones/index.json, and the index keeps what picks a section.
    let detail_tokens = tokens_estimate(&serde_json::to_string(&section_entries).map_err(|e| e.to_string())?);
    let light = detail_tokens > PART_TOKENS * 4;
    let mut grouped = false;
    let listed: Vec<serde_json::Value> = if light {
        std::fs::write(staging.join("secciones").join("index.json"), serde_json::to_string(&section_entries).map_err(|e| e.to_string())?).map_err(io)?;
        // One short line per section, not an object: field names and file
        // paths repeated for every section were most of a long index. A
        // level-2 section is indented under its level-1.
        section_entries.iter().map(|e| {
            let n = |k: &str| e.get(k).and_then(|v| v.as_u64());
            let count = |k: &str| e.get(k).and_then(|v| v.as_array()).map_or(0, Vec::len);
            let mut line = String::new();
            line.push_str(&"  ".repeat(n("level").unwrap_or(1).saturating_sub(1) as usize));
            line.push_str(e["id"].as_str().unwrap_or(""));
            match (n("page_start"), n("page_end")) {
                (Some(a), Some(b)) if a == b => line.push_str(&format!(" · p.{a}")),
                (Some(a), Some(b)) => line.push_str(&format!(" · p.{a}–{b}")),
                _ => {}
            }
            let s = |k: &str| e.get(k).and_then(|v| v.as_str());
            match (s("printed_start"), s("printed_end")) {
                (Some(x), Some(y)) if x != y => line.push_str(&format!(" (impr. {x}–{y})")),
                (Some(x), _) | (None, Some(x)) => line.push_str(&format!(" (impr. {x})")),
                _ => {}
            }
            line.push_str(&format!(" · {} tok", n("tokens_estimate").unwrap_or(0)));
            let (t, p) = (count("tables"), count("parts"));
            if t > 0 { line.push_str(&format!(" · {t} {}", if t == 1 { "tabla" } else { "tablas" })) }
            if p > 0 { line.push_str(&format!(" · {p} partes")) }
            if e.get("kind").and_then(|v| v.as_str()) == Some("toc") { line.push_str(" · índice") }
            line.push_str(" · ");
            line.push_str(e["title"].as_str().unwrap_or(""));
            serde_json::Value::String(line)
        }).collect::<Vec<_>>()
    } else { section_entries.clone() };
    // When even those lines cost more than a few parts (hundreds of
    // subsections), the index lists the top level with the range of its
    // subsections, and every line goes to secciones/lista.txt, to search.
    let lines_tokens = tokens_estimate(&serde_json::to_string(&listed).map_err(|e| e.to_string())?);
    let listed: Vec<serde_json::Value> = if light && lines_tokens > PART_TOKENS * 4 {
        let all: Vec<String> = listed.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        std::fs::write(staging.join("secciones").join("lista.txt"), all.join("\n") + "\n").map_err(io)?;
        let mut top: Vec<serde_json::Value> = Vec::new();
        let mut i = 0;
        while i < all.len() {
            if all[i].starts_with(' ') { i += 1; continue }
            let mut j = i + 1;
            while j < all.len() && all[j].starts_with(' ') { j += 1 }
            let mut line = all[i].clone();
            if j > i + 1 {
                let id = |l: &str| l.trim_start().split(' ').next().unwrap_or("").to_string();
                line.push_str(&format!(" · {} subsecciones {}–{}", j - i - 1, id(&all[i + 1]), id(&all[j - 1])));
            }
            top.push(serde_json::Value::String(line));
            i = j;
        }
        grouped = top.len() < all.len();
        if grouped { top } else { listed }
    } else { listed };
    let how = if grouped {
        "Este índice lista los capítulos; cada línea es «id · páginas · tokens · tablas · partes · título · subsecciones». La lista completa de secciones (una línea cada una) está en secciones/lista.txt: busca ahí el título con grep o usa docpack find, sin leerla entera. El archivo de la sección S491 es secciones/491-….md; su detalle (archivo, parts con líneas para leerla por partes, outline, tablas y figuras) está en secciones/index.json: busca ahí solo la sección elegida. Detalle de tablas en tablas/index.json y de figuras en figuras/index.json. El original solo si la persona lo pide."
    } else if light {
        "Lee este índice para elegir la sección: cada línea es «id · páginas · tokens · tablas · partes · título». El archivo de la sección S491 es secciones/491-….md; su detalle (archivo, parts con líneas para leerla por partes, outline, tablas y figuras) está en secciones/index.json: busca ahí solo la sección elegida. Detalle de tablas en tablas/index.json y de figuras en figuras/index.json. paginas/pNNN.png solo para ver una página. El original solo si la persona lo pide."
    } else {
        "Lee este índice; abre solo las secciones que necesites y, de ellas, sus tablas (tablas/Tnn.csv) y figuras. Una sección larga trae outline con líneas para leerla por partes. Detalle de tablas en tablas/index.json y de figuras en figuras/index.json. paginas/pNNN.png solo para ver una página. El original solo si la persona lo pide."
    };
    let file_name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let index = serde_json::json!({
        "colores": (!legend.is_empty()).then(|| doc.colors.entries().iter().map(|(kind, hex, u)| format!("{kind} {} {hex} ×{}", color::name(hex), u.count)).collect::<Vec<_>>()),
        "document": file_name, "kind": doc.kind, "pages": doc.pages,
        "how_to_read": how,
        "sections": listed,
        "section_detail": light.then_some("secciones/index.json"),
        "section_list": grouped.then_some("secciones/lista.txt"),
        "tables": table_entries.len(), "figures": figure_entries.len(),
        "page_images": match (page_images.is_empty(), only) {
            (true, _) => serde_json::Value::Null,
            (false, None) => serde_json::json!({"count": page_images.len(), "file": "paginas/pNNN.png"}),
            (false, Some(pages)) => serde_json::json!({"count": page_images.len(), "file": "paginas/pNNN.png", "only": pages}),
        },
    });
    let index = compact(index);
    std::fs::write(staging.join("tablas").join("index.json"), serde_json::to_string(&table_entries).map_err(|e| e.to_string())?).map_err(io)?;
    if !figure_entries.is_empty() { std::fs::write(staging.join("figuras").join("index.json"), serde_json::to_string(&figure_entries).map_err(|e| e.to_string())?).map_err(io)?; }
    let manifest = serde_json::json!({
        "docpack_version": VERSION,
        "source": {"file": source.to_string_lossy(), "name": file_name, "sha256": sha, "bytes": bytes.len(), "kind": doc.kind, "pages": doc.pages},
        "document": doc.about,
        "extractor": {"name": "docpack", "version": VERSION, "ocr": doc.ocr},
        "options": {"pages": options.pages},
        "stats": {"sections": section_entries.len(), "tables": table_entries.len(), "figures": figure_entries.len(), "page_images": page_images.len(), "tokens_total_estimate": total},
        "notes": doc.notes,
        "doubtful_pages": doc.doubtful_pages.iter().map(|(p, r)| (p.to_string(), *r)).collect::<std::collections::BTreeMap<_, _>>(),
        "colores": legend,
        "original_unchanged": true,
    });
    // The index is what the AI reads first: compact JSON, no indentation tokens.
    std::fs::write(staging.join("index.json"), serde_json::to_string(&index).map_err(|e| e.to_string())?).map_err(io)?;
    std::fs::write(staging.join("manifest.json"), serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(io)?;

    // The previous pack of this file is replaced only when it is a DocPack
    // pack: moved aside, the new one put in its place, then removed. A failure
    // in between puts the old one back, so the folder is never left without a
    // whole pack.
    if dir.exists() {
        if !dir.join("manifest.json").is_file() {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("{} existe y no es un pack de DocPack; no se sobrescribe", dir.display()));
        }
        let old = parent.join(format!(".{}.old-{}", dir.file_name().and_then(|n| n.to_str()).unwrap_or("pack"), std::process::id()));
        let _ = retry_io("borrar", &[&old], || if old.exists() { std::fs::remove_dir_all(&old) } else { Ok(()) });
        if let Err(e) = retry_io("mover el pack anterior", &[dir, &old], || std::fs::rename(dir, &old)) {
            // Nothing was replaced; the new extraction is kept to recover it.
            return Err(format!("{e}. El pack anterior sigue intacto; la extracción nueva quedó en {} (cierra lo que tenga abierta la carpeta, p. ej. el antivirus o un explorador, y repite)", staging.display()));
        }
        if let Err(e) = retry_io("publicar el pack nuevo", &[&staging, dir], || std::fs::rename(&staging, dir)) {
            let restored = retry_io("devolver el pack anterior", &[&old, dir], || std::fs::rename(&old, dir)).is_ok();
            return Err(format!("{e}. {}; la extracción nueva quedó en {}", if restored { "El pack anterior se devolvió a su lugar" } else { "OJO: el pack anterior quedó en la carpeta .old-* junto al pack" }, staging.display()));
        }
        let _ = retry_io("borrar", &[&old], || std::fs::remove_dir_all(&old));
    } else {
        retry_io("publicar el pack", &[&staging, dir], || std::fs::rename(&staging, dir)).map_err(|e| format!("{e}. La extracción quedó en {}", staging.display()))?;
    }
    // The folder's catalog, when the pack is in a project's docpack/.
    if parent.file_name().is_some_and(|n| n == "docpack") {
        if let Err(e) = library::write_catalog(parent) { eprintln!("docpack: no se pudo actualizar {}: {e}", parent.join("index.json").display()) }
    }
    Ok(Built { index: dir.join("index.json"), pack: dir.to_path_buf(), status: "built", sections: section_entries.len(), tables: table_entries.len(), page_images: page_images.len(), tokens_estimate: total })
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn a_busy_step_is_tried_again_until_it_works() {
        let mut calls = 0;
        let r = retry_io_with(&[1, 1, 1], "mover", &[Path::new("a")], || { calls += 1; if calls < 3 { Err(Error::from_raw_os_error(5)) } else { Ok(calls) } });
        assert_eq!(r, Ok(3));
    }

    #[test]
    fn a_step_that_stays_denied_names_the_operation_and_the_paths() {
        let mut calls = 0;
        let r: Result<(), String> = retry_io_with(&[1, 1], "mover el pack anterior", &[Path::new("pack"), Path::new(".pack.old-1")], || { calls += 1; Err(Error::new(ErrorKind::PermissionDenied, "Acceso denegado. (os error 5)")) });
        let e = r.unwrap_err();
        assert_eq!(calls, 3);
        assert!(e.contains("mover el pack anterior pack → .pack.old-1") && e.contains("os error 5") && e.contains("3 intentos"), "{e}");
    }

    #[test]
    fn other_errors_are_not_retried() {
        let mut calls = 0;
        let r: Result<(), String> = retry_io_with(&[1, 1], "leer", &[Path::new("x")], || { calls += 1; Err(Error::from(ErrorKind::NotFound)) });
        assert!(r.is_err());
        assert_eq!(calls, 1);
    }
}

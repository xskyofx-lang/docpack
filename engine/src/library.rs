//! What works on all the packs of a project (`<project>/docpack/`): the
//! catalog (`docpack/index.json`), search, comparing two versions of a
//! document, the health check and the text of chosen pages. Everything reads
//! the packs, never the originals (except the health check, which hashes them
//! to know whether a pack is still theirs).
use crate::{pack_dir, sha256, supported, VERSION};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// A pack's sections with all their detail: from secciones/index.json when a
/// long document keeps it there, else from its index.
fn section_list(dir: &Path, index: &Value) -> Vec<Value> {
    read_json(&dir.join("secciones").join("index.json")).and_then(|v| v.as_array().cloned())
        .unwrap_or_else(|| index["sections"].as_array().cloned().unwrap_or_default())
}

/// Text compared without case, accents or repeated spaces.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let c = match c { 'á' | 'à' | 'ä' | 'â' => 'a', 'é' | 'è' | 'ë' | 'ê' => 'e', 'í' | 'ì' | 'ï' | 'î' => 'i', 'ó' | 'ò' | 'ö' | 'ô' => 'o', 'ú' | 'ù' | 'ü' | 'û' => 'u', 'ñ' => 'n', c => c };
        if c.is_whitespace() { if !out.ends_with(' ') && !out.is_empty() { out.push(' ') } } else { out.push(c) }
    }
    out.trim_end().to_string()
}

/// The packs under `root`: folders with a manifest, by name.
fn packs(root: &Path) -> Vec<(PathBuf, Value)> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut out: Vec<(PathBuf, Value)> = entries.flatten().map(|e| e.path())
        .filter(|p| p.is_dir() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .filter_map(|p| read_json(&p.join("manifest.json")).map(|m| (p, m))).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn name_of(dir: &Path) -> String { dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() }

/// Writes `docpack/index.json`: every pack of the folder with its document's
/// title, version and date, its size and how current it is. Replaces the old
/// catalog in one step.
pub fn write_catalog(root: &Path) -> Result<PathBuf, String> {
    if !root.is_dir() { return Err(format!("Todavía no hay packs en {}: construye uno primero (docpack build).", root.display())) }
    let entries: Vec<Value> = packs(root).iter().map(|(dir, m)| {
        let about = &m["document"];
        crate::compact(json!({
            "pack": name_of(dir), "document": m["source"]["name"], "kind": m["source"]["kind"], "pages": m["source"]["pages"],
            "title": about["title"], "version": about["version"], "date": about["date"],
            "sections": m["stats"]["sections"], "tables": m["stats"]["tables"], "tokens_estimate": m["stats"]["tokens_total_estimate"],
            "sha256": m["source"]["sha256"], "docpack_version": m["docpack_version"],
        }))
    }).collect();
    let catalog = json!({
        "how_to_use": "Un pack por documento: abre <pack>/index.json. Para buscar en todos: docpack find \"texto\". Para comparar dos versiones: docpack diff <pack1> <pack2>.",
        "packs": entries,
    });
    let path = root.join("index.json");
    let temp = root.join(format!(".index.json.tmp-{}", std::process::id()));
    std::fs::write(&temp, serde_json::to_string(&catalog).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).map_err(|e| { let _ = std::fs::remove_file(&temp); e.to_string() })?;
    Ok(path)
}

/// The rows of one of our CSV files (quoted cells, "" for a quote).
pub fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let (mut rows, mut row, mut cell, mut quoted) = (Vec::new(), Vec::new(), String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => { cell.push('"'); chars.next(); }
            '"' => quoted = !quoted,
            ',' if !quoted => row.push(std::mem::take(&mut cell)),
            '\n' if !quoted => { row.push(std::mem::take(&mut cell)); rows.push(std::mem::take(&mut row)); }
            '\r' if !quoted => {}
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() { row.push(cell); rows.push(row); }
    rows
}

/// The page a section's line `n` (from 0) is on, and the number printed on
/// it when the document numbers its pages apart: its last page marker above
/// ("<!-- página 11 · impresa 9 -->"), else the page the section starts on.
fn page_at(lines: &[&str], n: usize, section: &Value) -> (Option<u64>, Option<String>) {
    let marker = lines[..=n].iter().rev().find_map(|l| l.strip_prefix("<!-- página ").map(|r| r.trim_end_matches("-->")));
    match marker.and_then(|r| Some((r.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?, r))) {
        Some((page, rest)) => (Some(page), rest.split(" · ").find_map(|p| p.strip_prefix("impresa ")).map(|p| p.trim().to_string())),
        None => (section["page_start"].as_u64(), section["printed_start"].as_str().map(str::to_string)),
    }
}

/// A table's hit with the number printed on its page, when the document
/// numbers its pages apart.
fn with_printed(mut hit: Value, table: &Value) -> Value {
    if let Some(p) = table["printed_page"].as_str() { hit["printed_page"] = json!(p) }
    hit
}

/// The words of a folded text, split where letters and digits stop.
fn tokens(text: &str) -> Vec<&str> { text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect() }

/// Every place in the packs under `root` that has all the words of `query`
/// (in one line of a section, or one row of a table): its pack, section or
/// table, part, page and line. A number matches a whole number; a word, the
/// start of a word ("ciclo" finds "ciclos"). The query as written comes
/// first, then the rest; tables of contents last. The first `limit`, and how
/// many there are.
pub fn find(root: &Path, query: &str, limit: usize) -> Value {
    find_with(root, query, &Find { limit, ..Default::default() })
}

/// What to search: how many hits to show, only packs whose name holds
/// `pack`, only "tablas" or only "texto", only what is marked in `color` (a
/// name such as "rosa" or a value such as "FF69B4").
#[derive(Debug, Clone, Default)]
pub struct Find { pub limit: usize, pub pack: Option<String>, pub only: Option<String>, pub color: Option<String> }

/// Whether a word of the text (folded) is `word`: a number whole, a word by
/// its start ("multa" finds "multas").
fn word_in(have: &[&str], word: &str) -> bool {
    have.iter().any(|h| if word.chars().all(|c| c.is_ascii_digit()) { h == &word } else { h.starts_with(word) })
}

/// Search every pack. The query may give several ways of saying one thing
/// apart with `|` ("multa | sanción | penalidad"): a place that says any of
/// them is a hit, and names the one it says (`matched`). Hits are ranked: the
/// words as written before the words apart, and in each a table's title, then
/// a heading, then body text, then a table's row; a table of contents last. A
/// line repeated word for word in one pack (a running header) is one hit with
/// its `count`. With `color`, only text or cells in that color, and the query
/// may be empty. Without hits, the answer says what the search could not see
/// (pages whose text may be incomplete, documents without a pack): an empty
/// search is not proof that a document does not say something.
pub fn find_with(root: &Path, query: &str, opts: &Find) -> Value {
    // Each way of saying it: as written (folded) and its words.
    let ways: Vec<(String, String, Vec<String>)> = query.split('|').map(str::trim).filter(|w| !w.is_empty())
        .map(|w| { let f = fold(w); let words = tokens(&f).into_iter().map(str::to_string).collect(); (w.to_string(), f, words) })
        .filter(|(_, _, words): &(String, String, Vec<String>)| !words.is_empty()).collect();
    let words: Vec<String> = ways.iter().flat_map(|w| w.2.clone()).collect();
    // (0: one way as written, 1: all its words; which way); None: no match.
    let rank = |text: &str| -> Option<(u8, usize)> {
        if ways.is_empty() { return Some((1, 0)) }
        let t = fold(text);
        let have = tokens(&t);
        ways.iter().enumerate().filter(|(_, (_, _, words))| words.iter().all(|w| word_in(&have, w)))
            .map(|(k, (_, folded, _))| (if t.contains(folded.as_str()) { 0 } else { 1 }, k)).min()
    };
    let matched = |hit: &mut Value, way: usize| if ways.len() > 1 { hit["matched"] = json!(ways[way].0) };
    // A color by its hex value or by its name, in Spanish or English
    // (`rosa` or `pink`): the pack names colors in Spanish.
    let asked_name = opts.color.as_deref().and_then(crate::color::named);
    let wanted = |hex: &str| opts.color.as_deref().is_none_or(|c| {
        let c = fold(c.trim_start_matches('#'));
        fold(hex) == c || fold(crate::color::name(hex)) == c || asked_name == Some(crate::color::name(hex))
    });
    let snippet = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(240).collect::<String>();
    let (texts, tables) = (opts.only.as_deref() != Some("tablas"), opts.only.as_deref() != Some("texto"));
    // (score, pack order, hit); a repeated line counts on its first hit.
    let mut hits: Vec<(u8, usize, Value)> = Vec::new();
    let mut seen: HashMap<(usize, String), usize> = HashMap::new();
    let mut add = |score: u8, order: usize, key: String, hit: Value, hits: &mut Vec<(u8, usize, Value)>| {
        if let Some(&at) = seen.get(&(order, key.clone())) {
            let count = hits[at].2["count"].as_u64().unwrap_or(1) + 1;
            hits[at].2["count"] = json!(count);
            return;
        }
        seen.insert((order, key), hits.len());
        hits.push((score, order, crate::compact(hit)));
    };
    for (order, (dir, _)) in packs(root).into_iter().enumerate() {
        let pack = name_of(&dir);
        if opts.pack.as_deref().is_some_and(|p| !fold(&pack).contains(&fold(p))) { continue }
        let Some(index) = read_json(&dir.join("index.json")) else { continue };
        if texts {
            for section in section_list(&dir, &index).iter() {
                let Some(file) = section["file"].as_str() else { continue };
                let Ok(body) = std::fs::read_to_string(dir.join(file)) else { continue };
                let lines: Vec<&str> = body.lines().collect();
                let contents = section["kind"] == "toc";
                for (n, line) in lines.iter().enumerate() {
                    if line.starts_with("<!--") { continue }
                    if opts.color.is_some() && !marked_colors(line).iter().any(|h| wanted(h)) { continue }
                    let plain = crate::color::strip_tags(line);
                    let Some((r, way)) = rank(&plain) else { continue };
                    let heading = plain.starts_with('#');
                    let leaders = contents || plain.contains("....");
                    let score = r * 4 + if heading { 1 } else { 2 } + if leaders { 8 } else { 0 };
                    let part = section["parts"].as_array().and_then(|parts| parts.iter().position(|p| p["lines"][0].as_u64().unwrap_or(0) <= n as u64 + 1 && p["lines"][1].as_u64().unwrap_or(0) >= n as u64 + 1)).map(|k| k + 1);
                    let (page, printed) = page_at(&lines, n, section);
                    let mut hit = json!({"pack": pack, "section": section["id"], "title": section["title"], "file": file, "line": n + 1, "part": part, "page": page, "text": snippet(&plain)});
                    if let Some(p) = printed { hit["printed_page"] = json!(p) }
                    matched(&mut hit, way);
                    add(score, order, fold(&plain), hit, &mut hits);
                }
            }
        }
        if tables {
            for table in read_json(&dir.join("tablas").join("index.json")).as_ref().and_then(Value::as_array).into_iter().flatten() {
                let Some(file) = table["file"].as_str() else { continue };
                let title = table["title"].as_str().unwrap_or("");
                // Its title is where a table is found by name.
                if opts.color.is_none() && !words.is_empty() {
                    if let Some((r, way)) = rank(title) {
                        let mut hit = with_printed(json!({"pack": pack, "table": table["id"], "title": title, "file": file, "page": table["page"], "section": table["section"], "sheet": table["sheet"], "text": snippet(title)}), table);
                        matched(&mut hit, way);
                        add(r * 4, order, format!("T:{}", table["id"]), hit, &mut hits);
                    }
                }
                let Ok(text) = std::fs::read_to_string(dir.join(file)) else { continue };
                let rows = csv_rows(&text);
                // A spreadsheet's table names its cells as the sheet does.
                let origin = table["origin"].as_str().and_then(cell_position);
                // With a color: the cells in it, with the pieces in that color.
                let colored: Option<Vec<Value>> = opts.color.as_ref().map(|_| table["colores_file"].as_str()
                    .and_then(|f| read_json(&dir.join(f))).and_then(|v| v.as_array().cloned()).unwrap_or_default());
                for (r, row) in rows.iter().enumerate() {
                    let joined = row.join(" | ");
                    let Some((score, way)) = rank(&joined) else { continue };
                    let mut hit = with_printed(json!({"pack": pack, "table": table["id"], "title": title, "file": file, "row": r + 1, "page": table["page"], "section": table["section"], "sheet": table["sheet"], "text": snippet(&joined)}), table);
                    // The cell that says it: the first with a word of the way
                    // that matched (column from 0, as in the CSV).
                    if let Some(c) = ways.get(way).and_then(|w| row.iter().position(|cell| { let f = fold(cell); let have = tokens(&f); w.2.iter().any(|x| word_in(&have, x)) })) {
                        hit["column"] = json!(c);
                        if let Some((r0, c0)) = origin { hit["cell"] = json!(cell_name(r0 + r as u32, c0 + c as u32)) }
                    }
                    matched(&mut hit, way);
                    if let Some(cells) = &colored {
                        let mut found = Vec::new();
                        for c in cells.iter().filter(|c| c["fila"].as_u64() == Some(r as u64)) {
                            let hex = |v: &Value| v.as_str().map(|s| s.split(' ').next().unwrap_or("").to_string());
                            let fill = hex(&c["fondo"]).filter(|h| wanted(h));
                            let pieces: Vec<String> = c["texto"].as_array().into_iter().flatten()
                                .filter(|m| hex(&m["color"]).is_some_and(|h| wanted(&h)) || hex(&m["resaltado"]).is_some_and(|h| wanted(&h)))
                                .filter_map(|m| m["texto"].as_str().map(str::to_string)).collect();
                            if fill.is_some() || !pieces.is_empty() {
                                found.push(crate::compact(json!({"columna": c["columna"], "fondo": fill, "texto": pieces})));
                            }
                        }
                        if found.is_empty() { continue }
                        hit["colores"] = json!(found);
                    }
                    add(score * 4 + 3, order, format!("R:{}:{r}", table["id"]), hit, &mut hits);
                }
            }
        }
    }
    hits.sort_by_key(|h| (h.0, h.1));
    let total = hits.len();
    let shown: Vec<Value> = hits.into_iter().take(opts.limit.max(1)).map(|h| h.2).collect();
    let mut out = json!({"query": query, "color": opts.color, "total": total, "shown": shown.len(), "hits": shown});
    if total == 0 { out["not_found"] = unseen(root, opts, ways.len()) }
    out
}

/// What an empty search could not see: the pages of the packs searched whose
/// text may be incomplete (read by OCR, without text, damaged), and the
/// project's documents without a pack. Said so that "it is not there" is
/// only concluded where the search could look.
fn unseen(root: &Path, opts: &Find, ways: usize) -> Value {
    let mut doubtful = serde_json::Map::new();
    let mut searched = 0;
    for (dir, manifest) in packs(root) {
        let pack = name_of(&dir);
        if opts.pack.as_deref().is_some_and(|p| !fold(&pack).contains(&fold(p))) { continue }
        searched += 1;
        if let Some(pages) = manifest["doubtful_pages"].as_object().filter(|p| !p.is_empty()) { doubtful.insert(pack, json!(pages)); }
    }
    let project = root.parent().unwrap_or(root);
    let without: Vec<String> = documents_without_pack(project).iter().map(|p| p.strip_prefix(project).unwrap_or(p).to_string_lossy().into_owned()).collect();
    let mut note = format!("Sin resultados en {searched} pack{}. Esto no prueba que el documento no lo diga:", if searched == 1 { "" } else { "s" });
    if ways == 1 { note.push_str(" busca también otras formas de decirlo (sinónimos, singular o plural, abreviaturas) separadas por |;") }
    if !doubtful.is_empty() { note.push_str(" en doubtful_pages hay páginas cuyo texto puede estar incompleto (ocr: leído por OCR; sin_texto: imagen sin texto legible; texto_dañado: letras que no se pudieron leer): revisa su imagen (paginas/pNNN.png) o su texto antes de concluir;") }
    if !without.is_empty() { note.push_str(" hay documentos sin pack (without_pack) que esta búsqueda no ve: constrúyelos y busca de nuevo;") }
    let note = note.trim_end_matches(';').to_string() + ".";
    crate::compact(json!({"note": note, "packs_searched": searched, "doubtful_pages": doubtful, "without_pack": without}))
}

/// The project's documents without a pack (hidden folders, files the
/// project's .docpackignore names and the packs themselves aside).
fn documents_without_pack(project: &Path) -> Vec<PathBuf> {
    let root = project.join("docpack");
    let mut out = Vec::new();
    let mut stack = vec![project.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if name_of(&path).starts_with('.') || path == root { continue }
            if path.is_dir() { stack.push(path) } else if supported(&path) && !ignored(project, &path) && !pack_dir(project, &path).join("manifest.json").is_file() { out.push(path) }
        }
    }
    out.sort();
    out
}

/// "D14" for a sheet's cell (row and column from 0).
pub fn cell_name(row: u32, column: u32) -> String {
    let (mut index, mut letters) = (column + 1, String::new());
    while index > 0 { let r = (index - 1) % 26; letters.insert(0, (b'A' + r as u8) as char); index = (index - 1) / 26; }
    format!("{letters}{}", row + 1)
}

/// "D14" → (row 13, column 3).
fn cell_position(name: &str) -> Option<(u32, u32)> {
    let letters: String = name.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let row: u32 = name[letters.len()..].parse().ok()?;
    let column = letters.to_ascii_uppercase().bytes().try_fold(0u32, |acc, b| acc.checked_mul(26)?.checked_add((b - b'A' + 1) as u32))?;
    Some((row.checked_sub(1)?, column.checked_sub(1)?))
}

/// The colors a section line marks with (`<span style="color:#…">`,
/// `<mark style="background:#…">`).
fn marked_colors(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for key in ["color:#", "background:#"] {
        let mut rest = line;
        while let Some(at) = rest.find(key) {
            let hex: String = rest[at + key.len()..].chars().take(6).collect();
            if hex.len() == 6 { out.push(hex) }
            rest = &rest[at + key.len()..];
        }
    }
    out
}

/// A pack's sections (folded title → text without page markers) and tables
/// (folded title and header → rows).
struct Loaded { about: Value, sections: Vec<(String, String, String)>, tables: Vec<(String, String, Vec<Vec<String>>)> }

fn load(dir: &Path) -> Result<Loaded, String> {
    let manifest = read_json(&dir.join("manifest.json")).ok_or_else(|| format!("{} no es un pack de DocPack", dir.display()))?;
    let index = read_json(&dir.join("index.json")).ok_or_else(|| format!("{} no tiene index.json", dir.display()))?;
    let mut sections = Vec::new();
    for s in section_list(dir, &index).iter() {
        let body = s["file"].as_str().and_then(|f| std::fs::read_to_string(dir.join(f)).ok()).unwrap_or_default();
        let text = body.lines().filter(|l| !l.starts_with("<!--") && !l.starts_with("_Página") && !l.trim().is_empty()).map(str::trim).collect::<Vec<_>>().join("\n");
        sections.push((s["id"].as_str().unwrap_or("").to_string(), s["title"].as_str().unwrap_or("").to_string(), text));
    }
    let mut tables = Vec::new();
    for t in read_json(&dir.join("tablas").join("index.json")).as_ref().and_then(Value::as_array).into_iter().flatten() {
        let rows = t["file"].as_str().and_then(|f| std::fs::read_to_string(dir.join(f)).ok()).map(|c| csv_rows(&c)).unwrap_or_default();
        tables.push((t["id"].as_str().unwrap_or("").to_string(), t["title"].as_str().unwrap_or("").to_string(), rows));
    }
    let about = json!({"pack": name_of(dir), "document": manifest["source"]["name"], "sha256": manifest["source"]["sha256"], "version": manifest["document"]["version"], "date": manifest["document"]["date"]});
    Ok(Loaded { about: crate::compact(about), sections, tables })
}

/// Pairs items of two lists by key, in order (the n-th "Alcance" of one with
/// the n-th of the other). Returns (pairs, only in a, only in b).
fn pair<K: Eq + std::hash::Hash + Clone>(a: &[K], b: &[K]) -> (Vec<(usize, usize)>, Vec<usize>, Vec<usize>) {
    let mut queue: HashMap<K, std::collections::VecDeque<usize>> = HashMap::new();
    for (j, k) in b.iter().enumerate() { queue.entry(k.clone()).or_default().push_back(j) }
    let (mut pairs, mut only_a) = (Vec::new(), Vec::new());
    for (i, k) in a.iter().enumerate() {
        match queue.get_mut(k).and_then(|q| q.pop_front()) { Some(j) => pairs.push((i, j)), None => only_a.push(i) }
    }
    let mut only_b: Vec<usize> = queue.into_values().flatten().collect();
    only_b.sort();
    (pairs, only_a, only_b)
}

/// The lines (or rows) of `a` missing from `b`, as many times as missing.
fn missing<T: Eq + std::hash::Hash + Clone>(a: &[T], b: &[T]) -> Vec<T> {
    let mut left: HashMap<&T, usize> = HashMap::new();
    for x in b { *left.entry(x).or_default() += 1 }
    a.iter().filter(|x| match left.get_mut(x) { Some(n) if *n > 0 => { *n -= 1; false } _ => true }).cloned().collect()
}

/// Rows shown per changed table, and lines per changed section; the counts
/// are always whole.
const ROWS_SHOWN: usize = 20;

/// What changed from the pack `a` to the pack `b`: sections added, removed or
/// with different text (matched by title), and tables added, removed or with
/// different rows (matched by title and header).
pub fn diff(a: &Path, b: &Path) -> Result<Value, String> {
    let (a, b) = (load(a)?, load(b)?);
    let keys = |l: &Loaded| l.sections.iter().map(|s| fold(&s.1)).collect::<Vec<_>>();
    let (pairs, removed, added) = pair(&keys(&a), &keys(&b));
    let changed: Vec<Value> = pairs.iter().filter(|(i, j)| a.sections[*i].2 != b.sections[*j].2).map(|(i, j)| {
        let (la, lb): (Vec<&str>, Vec<&str>) = (a.sections[*i].2.lines().collect(), b.sections[*j].2.lines().collect());
        let (gone, new) = (missing(&la, &lb), missing(&lb, &la));
        // The lines themselves, as a reader sees them: without DocPack's color
        // tags and page markers, and without blank lines.
        let shown = |lines: &[&str]| lines.iter().map(|l| crate::color::strip_tags(l).trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with("<!--")).take(ROWS_SHOWN).collect::<Vec<_>>();
        json!({"title": b.sections[*j].1, "a": a.sections[*i].0, "b": b.sections[*j].0,
               "lines_removed": gone.len(), "lines_added": new.len(), "removed": shown(&gone), "added": shown(&new)})
    }).collect();
    let table_key = |t: &(String, String, Vec<Vec<String>>)| format!("{}|{}", fold(&t.1), fold(&t.2.first().map(|r| r.join(" | ")).unwrap_or_default()));
    let (tpairs, tremoved, tadded) = pair(&a.tables.iter().map(table_key).collect::<Vec<_>>(), &b.tables.iter().map(table_key).collect::<Vec<_>>());
    let tchanged: Vec<Value> = tpairs.iter().filter(|(i, j)| a.tables[*i].2 != b.tables[*j].2).map(|(i, j)| {
        let (ra, rb) = (&a.tables[*i].2, &b.tables[*j].2);
        let (gone, new) = (missing(ra, rb), missing(rb, ra));
        json!({"title": b.tables[*j].1, "a": a.tables[*i].0, "b": b.tables[*j].0, "rows": [ra.len(), rb.len()],
               "rows_removed": gone.len(), "rows_added": new.len(),
               "removed": gone.iter().take(ROWS_SHOWN).map(|r| r.join(" | ")).collect::<Vec<_>>(),
               "added": new.iter().take(ROWS_SHOWN).map(|r| r.join(" | ")).collect::<Vec<_>>()})
    }).collect();
    let same = removed.is_empty() && added.is_empty() && changed.is_empty() && tremoved.is_empty() && tadded.is_empty() && tchanged.is_empty();
    let listed = |l: &Loaded, items: &[usize], tables: bool| items.iter().map(|&i| if tables { json!({"id": l.tables[i].0, "title": l.tables[i].1}) } else { json!({"id": l.sections[i].0, "title": l.sections[i].1}) }).collect::<Vec<_>>();
    Ok(json!({
        "a": a.about, "b": b.about, "same": same,
        "sections": {"added": listed(&b, &added, false), "removed": listed(&a, &removed, false), "changed": changed},
        "tables": {"added": listed(&b, &tadded, true), "removed": listed(&a, &tremoved, true), "changed": tchanged},
    }))
}

/// The health of a project's packs: builds left half done, folders without
/// a manifest, packs whose document changed or is gone, packs of an older
/// DocPack, files that are not packs, and documents with no pack yet.
pub fn doctor(project: &Path) -> Value {
    let root = project.join("docpack");
    let mut problems = Vec::new();
    let mut count = 0;
    let mut problem = |kind: &str, path: &Path, detail: String| problems.push(json!({"problem": kind, "path": path.to_string_lossy(), "detail": detail}));
    for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let path = entry.path();
        let name = name_of(&path);
        if name.starts_with('.') && (name.contains(".tmp-") || name.contains(".old-")) {
            problem("build_left_over", &path, "Restos de una construcción interrumpida: se pueden borrar.".into());
            continue;
        }
        if path.is_file() {
            if name != "index.json" { problem("foreign_file", &path, "Archivo que no es un pack dentro de docpack/.".into()) }
            continue;
        }
        let Some(manifest) = read_json(&path.join("manifest.json")) else {
            problem("incomplete", &path, "Carpeta sin manifest.json: no es un pack terminado.".into());
            continue;
        };
        count += 1;
        if !path.join("index.json").is_file() { problem("incomplete", &path, "Pack sin index.json.".into()); continue }
        let source = PathBuf::from(manifest["source"]["file"].as_str().unwrap_or(""));
        match std::fs::read(&source) {
            Err(_) => problem("source_missing", &path, format!("El documento {} ya no está.", source.display())),
            Ok(bytes) if manifest["source"]["sha256"].as_str() != Some(sha256(&bytes).as_str()) =>
                problem("stale", &path, format!("{} cambió después de crear el pack: vuelve a construirlo.", source.display())),
            Ok(_) if manifest["docpack_version"].as_str() != Some(VERSION) =>
                problem("old_version", &path, format!("Hecho con DocPack {}; se rehace con {VERSION} la próxima vez que se lea.", manifest["docpack_version"].as_str().unwrap_or("?"))),
            Ok(_) => {}
        }
    }
    for path in documents_without_pack(project) { problem("no_pack", &path, "Documento sin pack.".into()) }
    json!({"packs": count, "problems": problems.len(), "list": problems})
}

/// Page numbers from "4-5", "7" or "4-5,9".
pub fn parse_pages(spec: &str) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once(['-', '–']).unwrap_or((part, part));
        let (a, b): (u32, u32) = (a.trim().parse().map_err(|_| format!("Página inválida: {part}"))?, b.trim().parse().map_err(|_| format!("Página inválida: {part}"))?);
        if a == 0 || b < a { return Err(format!("Rango inválido: {part}")) }
        out.extend(a..=b);
    }
    if out.is_empty() { return Err("Faltan las páginas (por ejemplo 4-5)".into()) }
    out.sort();
    out.dedup();
    Ok(out)
}

/// What to read from a pack: its index (nothing named), a section (whole, or
/// one of its parts) or a table (whole, or some of its rows).
#[derive(Debug, Default, Clone)]
pub struct Read { pub section: Option<String>, pub part: Option<usize>, pub table: Option<String>, pub rows: Option<String> }

/// An id as the pack writes it ("S5", "s05" and "S05" are one).
fn same_id(given: &str, id: &str) -> bool {
    let norm = |t: &str| {
        let t = t.trim().to_uppercase();
        match t.split_at_checked(1) {
            Some((k, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => format!("{k}{}", n.trim_start_matches('0')),
            _ => t,
        }
    };
    norm(given) == norm(id)
}

/// The entry `given` names, by id or by title: the title as written first,
/// then one that holds it.
fn entry_named<'a>(entries: &'a [Value], given: &str) -> Option<&'a Value> {
    let title = |e: &Value| fold(e["title"].as_str().unwrap_or(""));
    let wanted = fold(given);
    entries.iter().find(|e| e["id"].as_str().is_some_and(|id| same_id(given, id)))
        .or_else(|| entries.iter().find(|e| title(e) == wanted))
        .or_else(|| entries.iter().find(|e| !wanted.is_empty() && title(e).contains(&wanted)))
}

/// "S01 · título" for each entry, to say what there is when a name matches
/// nothing.
fn names(entries: &[Value]) -> String {
    let shown: Vec<String> = entries.iter().take(40).map(|e| format!("{} · {}", e["id"].as_str().unwrap_or(""), e["title"].as_str().unwrap_or(""))).collect();
    let more = if entries.len() > 40 { format!("\n… y {} más", entries.len() - 40) } else { String::new() };
    format!("{}{more}", shown.join("\n"))
}

/// " · p. 5 (impresa 3)", " · p. 5–7", or nothing without pages.
fn pages_label(start: Option<u64>, end: Option<u64>, printed: Option<&str>) -> String {
    let printed = printed.map(|p| format!(" (impresa {p})")).unwrap_or_default();
    match (start, end) {
        (Some(a), Some(b)) if a != b => format!(" · p. {a}–{b}{printed}"),
        (Some(a), _) => format!(" · p. {a}{printed}"),
        _ => String::new(),
    }
}

/// A piece of a pack, for an agent that reads only through DocPack (an MCP
/// client without a file reader): the index, a section or one of its parts,
/// or a table with its colors and formulas. Each piece begins with a line that
/// says what it is and where it comes from.
pub fn read(pack: &Path, what: &Read) -> Result<String, String> {
    let index_text = std::fs::read_to_string(pack.join("index.json")).map_err(|_| format!("{} no tiene index.json", pack.display()))?;
    let index: Value = serde_json::from_str(&index_text).map_err(|e| format!("index.json ilegible: {e}"))?;
    if let Some(name) = what.table.as_deref() {
        let tables = read_json(&pack.join("tablas").join("index.json")).and_then(|v| v.as_array().cloned()).unwrap_or_default();
        let table = entry_named(&tables, name).ok_or_else(|| format!("No hay tabla «{name}». Tablas del documento:\n{}", names(&tables)))?;
        let file = table["file"].as_str().ok_or("La tabla no tiene archivo")?;
        let rows = csv_rows(&std::fs::read_to_string(pack.join(file)).map_err(|e| format!("{file}: {e}"))?);
        let header = table["header_row"].as_u64().unwrap_or(0) as usize;
        // Rows as `find` names them (from 1, the header among them); the
        // header row always goes first so the columns keep their names.
        let chosen: Vec<usize> = match what.rows.as_deref() {
            Some(spec) => {
                let asked = parse_pages(spec).map_err(|e| e.replace("Página", "Fila").replace("páginas", "filas"))?;
                let mut picked: Vec<usize> = std::iter::once(header).chain(asked.iter().map(|r| *r as usize - 1)).filter(|r| *r < rows.len()).collect();
                picked.sort();
                picked.dedup();
                picked
            }
            None if table["read"].is_string() => return Err(format!("{} es una tabla de datos de {} filas: pide rows (por ejemplo \"1-50\") o búscala con docpack_find. Encabezado: {}", table["id"].as_str().unwrap_or(""), rows.len(), table["header"])),
            None => (0..rows.len()).collect(),
        };
        let last_page = table["pages"].as_array().and_then(|p| p.last()).and_then(Value::as_u64);
        let shown = if chosen.len() < rows.len() { format!(", se muestran {}", chosen.len()) } else { String::new() };
        // A sheet's table names its sheet, the cell it starts at, and whether
        // the sheet is hidden or protected.
        let sheet = table["sheet"].as_str().map(|s| format!(" · hoja «{s}» desde {}{}{}", table["origin"].as_str().unwrap_or("A1"),
            if table["hidden"] == true { " · oculta" } else { "" }, if table["protected"] == true { " · protegida" } else { "" })).unwrap_or_default();
        let mut out = format!("<!-- {} · {}{}{sheet} · {} filas{shown} · {file} -->\n", table["id"].as_str().unwrap_or(""), table["title"].as_str().unwrap_or(""),
            pages_label(table["page"].as_u64(), last_page.or(table["page"].as_u64()), table["printed_page"].as_str()), rows.len());
        for &r in &chosen {
            out.push_str(&format!("{}: {}\n", r + 1, rows[r].iter().map(|c| crate::csv_cell(c)).collect::<Vec<_>>().join(",")));
        }
        // Its formulas, colors and review findings, for the rows shown (fila
        // from 0, as in the CSV; celda as the sheet names it).
        for (key, label) in [("formulas_file", "Fórmulas"), ("colores_file", "Colores"), ("review_file", "Para revisar")] {
            let Some(cells) = table[key].as_str().and_then(|f| read_json(&pack.join(f))).and_then(|v| v.as_array().cloned()) else { continue };
            let picked: Vec<&Value> = cells.iter().filter(|c| c["fila"].as_u64().is_some_and(|f| chosen.contains(&(f as usize)))).collect();
            if !picked.is_empty() { out.push_str(&format!("\n{label} (fila y columna desde 0, como en el CSV):\n{}\n", serde_json::to_string(&picked).unwrap_or_default())) }
        }
        return Ok(out);
    }
    if let Some(name) = what.section.as_deref() {
        let sections = section_list(pack, &index);
        let section = entry_named(&sections, name).ok_or_else(|| format!("No hay sección «{name}». Secciones del documento:\n{}", names(&sections)))?;
        let file = section["file"].as_str().ok_or("La sección no tiene archivo")?;
        let body = std::fs::read_to_string(pack.join(file)).map_err(|e| format!("{file}: {e}"))?;
        let parts = section["parts"].as_array().cloned().unwrap_or_default();
        let head = format!("<!-- {} · {}{}", section["id"].as_str().unwrap_or(""), section["title"].as_str().unwrap_or(""),
            pages_label(section["page_start"].as_u64(), section["page_end"].as_u64(), section["printed_start"].as_str()));
        let tables = section["tables"].as_array().filter(|t| !t.is_empty())
            .map(|t| format!("\nTablas de esta sección (léelas con table): {}", t.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))).unwrap_or_default();
        return match what.part {
            // A long section is read by parts: without one, the parts to pick.
            None if !parts.is_empty() => {
                let list: Vec<String> = parts.iter().enumerate().map(|(i, p)| format!("parte {} · líneas {}–{} · {} tok · {}", i + 1, p["lines"][0], p["lines"][1], p["tokens_estimate"], p["starts"].as_str().unwrap_or(""))).collect();
                Ok(format!("{head} · {} tok -->\nSección larga: pide una parte con part (1 a {}).\n{}{tables}\n", section["tokens_estimate"], parts.len(), list.join("\n")))
            }
            None => Ok(format!("{head} · {file} -->\n{}{tables}\n", body.trim_end())),
            Some(k) => {
                let p = parts.get(k.wrapping_sub(1)).ok_or_else(|| if parts.is_empty() { "Esta sección no tiene partes: léela sin part".to_string() } else { format!("La sección tiene las partes 1 a {}", parts.len()) })?;
                let lines: Vec<&str> = body.lines().collect();
                let a = (p["lines"][0].as_u64().unwrap_or(1) as usize).clamp(1, lines.len().max(1));
                let b = (p["lines"][1].as_u64().unwrap_or(1) as usize).clamp(a, lines.len().max(a));
                let text = lines.get(a - 1..b.min(lines.len())).unwrap_or_default().join("\n");
                // The page the part starts on, when its first line is not a
                // page marker: the last marker above it.
                let page = lines[..a - 1].iter().rev().find(|l| l.starts_with("<!-- página ")).filter(|_| !text.starts_with("<!-- página ")).map(|l| format!("\n{l}")).unwrap_or_default();
                Ok(format!("{head} · parte {k} de {} · líneas {a}–{b} -->{page}\n{text}{tables}\n", parts.len()))
            }
        };
    }
    // The index, with the limits the manifest states (OCR, tables read by
    // alignment, printed page numbers): an agent that reads only through
    // DocPack sees them too.
    let notes = read_json(&pack.join("manifest.json")).map(|m| m["notes"].clone()).filter(|n| n.as_array().is_some_and(|a| !a.is_empty()));
    match (notes, index) {
        (Some(notes), Value::Object(mut map)) => {
            map.insert("notes".into(), notes);
            serde_json::to_string(&map).map_err(|e| e.to_string())
        }
        _ => Ok(index_text),
    }
}

/// The text of `pages` from a pack: each page under its marker. A document
/// whose sections only know the page they start and end on (Word) gives the
/// sections that cover those pages.
pub fn page_text(pack: &Path, pages: &[u32]) -> Result<String, String> {
    let index = read_json(&pack.join("index.json")).ok_or_else(|| format!("{} no tiene index.json", pack.display()))?;
    let mut out = String::new();
    let mut any_pages = false;
    for s in section_list(pack, &index).iter() {
        let (Some(start), Some(end)) = (s["page_start"].as_u64(), s["page_end"].as_u64()) else { continue };
        any_pages = true;
        if !pages.iter().any(|&p| p as u64 >= start && p as u64 <= end) { continue }
        let Some(body) = s["file"].as_str().and_then(|f| std::fs::read_to_string(pack.join(f)).ok()) else { continue };
        if body.contains("<!-- página ") {
            let mut keep = false;
            for line in body.lines() {
                if let Some(n) = line.strip_prefix("<!-- página ").and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()?.parse::<u32>().ok()) { keep = pages.contains(&n) }
                if keep { out.push_str(line); out.push('\n') }
            }
        } else {
            out.push_str(&format!("<!-- {} · páginas {start}–{end} -->\n{}\n", s["id"].as_str().unwrap_or(""), body.trim()));
        }
    }
    if !any_pages { return Err("Este pack no tiene números de página (Word sin motor de paginación en este equipo): lee sus secciones en index.json".into()) }
    if out.is_empty() { return Err(format!("El documento no tiene esas páginas (tiene {})", index["pages"])) }
    Ok(out)
}

/// Whether `path` matches a pattern of the project's `.docpackignore` (one
/// per line, `#` comments; `*` any letters but `/`, `**` anything, `?` one
/// letter), against its file name or its path from the project. Such files
/// get no pack: data exports, logs.
pub fn ignored(project: &Path, path: &Path) -> bool {
    let Ok(rules) = std::fs::read_to_string(project.join(".docpackignore")) else { return false };
    let name = name_of(path);
    let relative = match (project.canonicalize(), path.canonicalize()) {
        (Ok(p), Ok(f)) => f.strip_prefix(&p).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
        _ => path.strip_prefix(project).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
    };
    rules.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'))
        .any(|rule| glob(rule.to_lowercase().as_bytes(), name.to_lowercase().as_bytes()) || (!relative.is_empty() && glob(rule.to_lowercase().as_bytes(), relative.to_lowercase().as_bytes())))
}

fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.first() {
        None => text.is_empty(),
        Some(b'*') if pattern.get(1) == Some(&b'*') => (0..=text.len()).any(|i| glob(&pattern[2..], &text[i..])),
        Some(b'*') => (0..=text.len()).take_while(|&i| i == 0 || text[i - 1] != b'/').any(|i| glob(&pattern[1..], &text[i..])),
        Some(b'?') => !text.is_empty() && text[0] != b'/' && glob(&pattern[1..], &text[1..]),
        Some(c) => text.first() == Some(c) && glob(&pattern[1..], &text[1..]),
    }
}

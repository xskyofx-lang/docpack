//! PDF: text page by page, read from where each character is drawn, and
//! tables from the lines the page draws around them. Sections come from the
//! document's own bookmarks (outline) when it has them, levels 1-2, each
//! covering the pages up to the next one; without bookmarks every page is a
//! section. Page markers stay in the text so any line can be cited by page.
//!
//! A table is a grid of ruling lines (borders) with text in at least two of
//! its cells; its characters go to the CSV and leave the running text, which
//! keeps a reference where the table was. A table without drawn borders has
//! nothing on the page that says it is a table, and stays as text.
use crate::{Document, Section, Table};
use lopdf::{content::Content, Dictionary, Object};
use pdf_extract::{MediaBox, OutputDev, OutputError, Transform};

/// A drawn character: its baseline origin, advance and size, in page space.
#[derive(Debug, Clone)]
struct Glyph { x: f64, y: f64, advance: f64, size: f64, text: String, word_start: bool }

/// A horizontal (y, x0, x1) or vertical (x, y0, y1) ruling segment.
type Segment = (f64, f64, f64);

#[derive(Default)]
struct Page { glyphs: Vec<Glyph>, horizontal: Vec<Segment>, vertical: Vec<Segment>, areas: Vec<Area> }

/// A filled area (x0, y0, x1, y1): a cell's background, never a line by itself.
type Area = [f64; 4];

/// Receives the characters pdf-extract positions. The lines are read apart
/// (`Rules`): pdf-extract leaves even-odd fills (`f*`, how Word draws every
/// border) unpainted.
#[derive(Default)]
struct Collector { pages: Vec<Page>, word_start: bool }

impl OutputDev for Collector {
    fn begin_page(&mut self, _n: u32, _media: &MediaBox, _art: Option<(f64, f64, f64, f64)>) -> Result<(), OutputError> { self.pages.push(Page::default()); Ok(()) }
    fn end_page(&mut self) -> Result<(), OutputError> { Ok(()) }
    fn output_character(&mut self, trm: &Transform, width: f64, spacing: f64, font_size: f64, text: &str) -> Result<(), OutputError> {
        // The size of a square with the area the font size takes on the page.
        let (vx, vy) = (font_size * (trm.m11 + trm.m21), font_size * (trm.m12 + trm.m22));
        let size = (vx * vy).abs().sqrt();
        // The advance includes the character spacing (Tc) the text sets: letters
        // spread apart are still one word.
        let scale = if font_size != 0.0 { size / font_size.abs() } else { 0.0 };
        let word_start = std::mem::take(&mut self.word_start);
        if self.pages.is_empty() { self.pages.push(Page::default()) }
        self.pages.last_mut().unwrap().glyphs.push(Glyph { x: trm.m31, y: trm.m32, advance: width * size + spacing * scale, size, text: text.to_string(), word_start });
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> { self.word_start = true; Ok(()) }
    fn end_word(&mut self) -> Result<(), OutputError> { Ok(()) }
    fn end_line(&mut self) -> Result<(), OutputError> { Ok(()) }
}

/// A PDF matrix [a b c d e f]: x' = a·x + c·y + e, y' = b·x + d·y + f.
type Matrix = [f64; 6];
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
fn times(m: &Matrix, n: &Matrix) -> Matrix {
    [m[0] * n[0] + m[1] * n[2], m[0] * n[1] + m[1] * n[3], m[2] * n[0] + m[3] * n[2], m[2] * n[1] + m[3] * n[3], m[4] * n[0] + m[5] * n[2] + n[4], m[4] * n[1] + m[5] * n[3] + n[5]]
}
fn point(m: &Matrix, x: f64, y: f64) -> (f64, f64) { (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]) }
fn number(o: &Object) -> f64 { o.as_float().map(f64::from).or_else(|_| o.as_i64().map(|i| i as f64)).unwrap_or(0.0) }

/// One piece of a path being built: a line, or a rectangle (`re`).
enum Piece { Line((f64, f64), (f64, f64)), Rect([(f64, f64); 4]) }

/// The ruling lines a page paints: every stroke and fill, in page space,
/// through `cm` and the form XObjects it draws.
struct Rules<'a> { pdf: &'a lopdf::Document, horizontal: Vec<Segment>, vertical: Vec<Segment>, areas: Vec<Area> }

impl<'a> Rules<'a> {
    fn line(&mut self, (x0, y0): (f64, f64), (x1, y1): (f64, f64)) {
        if (y0 - y1).abs() <= SNAP && (x0 - x1).abs() > SNAP { self.horizontal.push(((y0 + y1) / 2.0, x0.min(x1), x0.max(x1))) }
        else if (x0 - x1).abs() <= SNAP && (y0 - y1).abs() > SNAP { self.vertical.push(((x0 + x1) / 2.0, y0.min(y1), y0.max(y1))) }
    }
    /// A stroke draws every edge; a fill draws a line only when it is thin
    /// (a fill that covers an area is a background, not a border).
    fn paint(&mut self, pieces: &[Piece], filled: bool, stroked: bool) {
        for piece in pieces {
            match piece {
                Piece::Line(a, b) => if stroked { self.line(*a, *b) },
                Piece::Rect(c) => {
                    let (x0, x1) = (c.iter().map(|p| p.0).fold(f64::MAX, f64::min), c.iter().map(|p| p.0).fold(f64::MIN, f64::max));
                    let (y0, y1) = (c.iter().map(|p| p.1).fold(f64::MAX, f64::min), c.iter().map(|p| p.1).fold(f64::MIN, f64::max));
                    // A thin filled rectangle is how most writers draw a border.
                    if filled && y1 - y0 <= THIN { self.line((x0, (y0 + y1) / 2.0), (x1, (y0 + y1) / 2.0)) }
                    else if filled && x1 - x0 <= THIN { self.line(((x0 + x1) / 2.0, y0), ((x0 + x1) / 2.0, y1)) }
                    else if stroked { self.line((x0, y0), (x1, y0)); self.line((x0, y1), (x1, y1)); self.line((x0, y0), (x0, y1)); self.line((x1, y0), (x1, y1)) }
                    else { self.areas.push([x0, y0, x1, y1]) }
                }
            }
        }
    }
    fn form(&self, resources: Option<&'a Dictionary>, name: &[u8]) -> Option<&'a lopdf::Stream> {
        let pdf: &'a lopdf::Document = self.pdf;
        let xobjects = resources?.get(b"XObject").ok().and_then(|x| pdf.dereference(x).ok())?.1.as_dict().ok()?;
        let stream = pdf.dereference(xobjects.get(name).ok()?).ok()?.1.as_stream().ok()?;
        (stream.dict.get(b"Subtype").and_then(|s| s.as_name()).ok() == Some(b"Form".as_slice())).then_some(stream)
    }
    fn walk(&mut self, content: &[u8], resources: Option<&'a Dictionary>, start: Matrix, depth: u8) {
        let Ok(content) = Content::decode(content) else { return };
        let mut stack: Vec<Matrix> = Vec::new();
        let mut ctm = start;
        let mut pieces: Vec<Piece> = Vec::new();
        let (mut at, mut first) = ((0.0, 0.0), (0.0, 0.0));
        for op in &content.operations {
            let n: Vec<f64> = op.operands.iter().map(number).collect();
            match op.operator.as_str() {
                "q" => stack.push(ctm),
                "Q" => ctm = stack.pop().unwrap_or(start),
                "cm" if n.len() == 6 => ctm = times(&[n[0], n[1], n[2], n[3], n[4], n[5]], &ctm),
                "m" if n.len() == 2 => { at = point(&ctm, n[0], n[1]); first = at }
                "l" if n.len() == 2 => { let to = point(&ctm, n[0], n[1]); pieces.push(Piece::Line(at, to)); at = to }
                "c" if n.len() == 6 => at = point(&ctm, n[4], n[5]),
                "v" | "y" if n.len() == 4 => at = point(&ctm, n[2], n[3]),
                "h" => { pieces.push(Piece::Line(at, first)); at = first }
                "re" if n.len() == 4 => {
                    let (x, y, w, h) = (n[0], n[1], n[2], n[3]);
                    pieces.push(Piece::Rect([point(&ctm, x, y), point(&ctm, x + w, y), point(&ctm, x + w, y + h), point(&ctm, x, y + h)]));
                    at = point(&ctm, x, y);
                    first = at;
                }
                "S" | "s" => { self.paint(&pieces, false, true); pieces.clear() }
                "f" | "F" | "f*" => { self.paint(&pieces, true, false); pieces.clear() }
                "B" | "B*" | "b" | "b*" => { self.paint(&pieces, true, true); pieces.clear() }
                "n" => pieces.clear(),
                "Do" if depth < 8 => {
                    let Some(name) = op.operands.first().and_then(|o| o.as_name().ok()) else { continue };
                    let Some(stream) = self.form(resources, name) else { continue };
                    let matrix = stream.dict.get(b"Matrix").and_then(|m| m.as_array()).ok().filter(|m| m.len() == 6)
                        .map(|m| [number(&m[0]), number(&m[1]), number(&m[2]), number(&m[3]), number(&m[4]), number(&m[5])]).unwrap_or(IDENTITY);
                    let pdf: &'a lopdf::Document = self.pdf;
                    let inner = stream.dict.get(b"Resources").ok().and_then(|r| pdf.dereference(r).ok()).and_then(|(_, r)| r.as_dict().ok()).or(resources);
                    let bytes = stream.decompressed_content().unwrap_or_else(|_| stream.content.clone());
                    self.walk(&bytes, inner, times(&matrix, &ctm), depth + 1);
                }
                _ => {}
            }
        }
    }
}

fn rules(pdf: &lopdf::Document, page: lopdf::ObjectId) -> (Vec<Segment>, Vec<Segment>, Vec<Area>) {
    let mut rules = Rules { pdf, horizontal: Vec::new(), vertical: Vec::new(), areas: Vec::new() };
    let Ok(content) = pdf.get_page_content(page) else { return Default::default() };
    let resources = pdf.get_page_resources(page).ok().and_then(|(own, _)| own);
    rules.walk(&content, resources, IDENTITY, 0);
    (rules.horizontal, rules.vertical, rules.areas)
}

/// Points within which two lines are the same line (a border drawn twice,
/// half a point of rounding between writers).
const SNAP: f64 = 1.5;
/// A filled rectangle at most this thick (points) is a drawn border.
const THIN: f64 = 3.0;

/// A table found on a page: its column and row lines, and its glyphs by cell.
struct Grid { xs: Vec<f64>, ys: Vec<f64>, rows: Vec<Vec<String>>, members: Vec<usize> }

fn cluster(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for v in values { if out.last().is_none_or(|l| v - l > SNAP * 2.0) { out.push(v) } }
    out
}

/// Ruling segments that touch each other, merged into tables.
fn grids(page: &Page) -> Vec<Grid> {
    let (h, v) = (&page.horizontal, &page.vertical);
    let n = h.len() + v.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize { while p[i] != i { p[i] = p[p[i]]; i = p[i] } i }
    for (i, &(y, x0, x1)) in h.iter().enumerate() {
        for (j, &(x, y0, y1)) in v.iter().enumerate() {
            if x >= x0 - SNAP && x <= x1 + SNAP && y >= y0 - SNAP && y <= y1 + SNAP {
                let (a, b) = (root(&mut parent, i), root(&mut parent, h.len() + j));
                parent[a] = b;
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, (Vec<usize>, Vec<usize>)> = Default::default();
    for i in 0..n {
        let r = root(&mut parent, i);
        let g = groups.entry(r).or_default();
        if i < h.len() { g.0.push(i) } else { g.1.push(i - h.len()) }
    }
    let mut out = Vec::new();
    for (hs, vs) in groups.into_values() {
        if hs.len() < 2 || vs.len() < 2 { continue }
        let xs = cluster(vs.iter().map(|&j| v[j].0).collect());
        let mut ys = cluster(hs.iter().map(|&i| h[i].0).collect());
        ys.reverse(); // top to bottom
        if xs.len() < 2 || ys.len() < 2 || (xs.len() - 1) * (ys.len() - 1) < 2 { continue }
        // A column line counts for a row only where something drawn separates
        // the cells there: a border, or the edge of a cell's background at a
        // column the borders already set (a shaded header without inner
        // borders). A cell merged across columns has neither inside it; one
        // merged down has no line under its first row.
        let near = |a: f64, b: f64| (a - b).abs() <= SNAP * 2.0;
        let has_wall = |x: f64, top: f64, bottom: f64| {
            let mid = (top + bottom) / 2.0;
            vs.iter().any(|&j| near(v[j].0, x) && v[j].1 <= mid && v[j].2 >= mid)
                || page.areas.iter().any(|a| (near(a[0], x) || near(a[2], x)) && a[1] <= mid && a[3] >= mid)
        };
        let has_floor = |y: f64, x: f64| {
            hs.iter().any(|&i| near(h[i].0, y) && h[i].1 <= x && h[i].2 >= x)
                || page.areas.iter().any(|a| (near(a[1], y) || near(a[3], y)) && a[0] <= x && a[2] >= x)
        };
        let mut cells: Vec<Vec<Vec<usize>>> = vec![vec![Vec::new(); xs.len() - 1]; ys.len() - 1];
        let mut members = Vec::new();
        for (g, glyph) in page.glyphs.iter().enumerate() {
            let (cx, cy) = (glyph.x + glyph.advance / 2.0, glyph.y + glyph.size * 0.3);
            if cx < xs[0] || cx > *xs.last().unwrap() || cy > ys[0] || cy < *ys.last().unwrap() { continue }
            let Some(mut row) = (0..ys.len() - 1).find(|&r| cy <= ys[r] && cy >= ys[r + 1]) else { continue };
            let Some(mut col) = (0..xs.len() - 1).find(|&c| cx >= xs[c] && cx <= xs[c + 1]) else { continue };
            while row > 0 && !has_floor(ys[row], cx) { row -= 1 }
            while col > 0 && !has_wall(xs[col], ys[row], ys[row + 1]) { col -= 1 }
            cells[row][col].push(g);
            members.push(g);
        }
        let filled = cells.iter().flatten().filter(|c| c.iter().any(|&g| !page.glyphs[g].text.trim().is_empty())).count();
        if filled < 2 { continue }
        // A cell spans from its column line to the next wall on its row.
        let rows: Vec<Vec<String>> = cells.iter().enumerate().map(|(r, row)| row.iter().enumerate().map(|(c, glyphs)| {
            let right = (c + 1..xs.len()).find(|&k| k == xs.len() - 1 || has_wall(xs[k], ys[r], ys[r + 1])).map(|k| xs[k]).unwrap_or(xs[c + 1]);
            cell_text(page, glyphs, xs[c], right)
        }).collect::<Vec<_>>()).filter(|r| r.iter().any(|c| !c.is_empty())).collect();
        out.push(Grid { xs, ys, rows, members });
    }
    out
}

/// The text of one cell (between the walls `left` and `right`): its glyphs in
/// reading order, a line per baseline.
fn cell_text(page: &Page, glyphs: &[usize], left: f64, right: f64) -> String {
    let mut gs: Vec<&Glyph> = glyphs.iter().map(|&g| &page.glyphs[g]).collect();
    gs.sort_by(|a, b| b.y.total_cmp(&a.y));
    let mut lines: Vec<Vec<&Glyph>> = Vec::new();
    for g in gs {
        match lines.last_mut() {
            Some(line) if (line[0].y - g.y).abs() <= g.size * 0.5 => line.push(g),
            _ => lines.push(vec![g]),
        }
    }
    // (text, right end, left start, advance of its first glyph)
    let lines: Vec<(String, f64, f64, f64)> = lines.iter_mut().map(|line| {
        line.sort_by(|a, b| a.x.total_cmp(&b.x));
        let mut text = String::new();
        let mut end = f64::MIN;
        let visible: Vec<&&Glyph> = line.iter().filter(|g| !g.text.trim().is_empty()).collect();
        let (start, first) = visible.first().map(|g| (g.x, g.advance)).unwrap_or((left, 0.0));
        for g in line.iter() {
            if !text.is_empty() && g.x > end + g.size * 0.1 && !text.ends_with(' ') { text.push(' ') }
            text.push_str(&g.text);
            if !g.text.trim().is_empty() { end = g.x + g.advance }
        }
        (text, end, start, first)
    }).filter(|(t, ..)| !t.trim().is_empty()).collect();
    // A word too long for its cell is split by the layout with no space at the
    // end of the line, and the line is full: what is left up to the right wall
    // (minus the cell's padding, measured on its left side) does not fit the
    // first letter of the next line. Then the two lines are one word.
    let padding = lines.iter().map(|l| l.2 - left).fold(f64::MAX, f64::min).max(0.0);
    let mut out = String::new();
    for (i, (text, end, ..)) in lines.iter().enumerate() {
        out.push_str(&text.split_whitespace().collect::<Vec<_>>().join(" "));
        if let Some(next) = lines.get(i + 1) {
            let full = right - padding - end < next.3;
            if text.ends_with(char::is_whitespace) || !full { out.push('\n') }
        }
    }
    out
}

/// Running text of a page in drawing order (the order pdf-extract's own
/// plain text uses), with the tables' characters replaced by a reference.
fn page_text(page: &Page, table_of: &std::collections::HashMap<usize, usize>) -> String {
    let mut out = String::new();
    let (mut last_y, mut last_end, mut first) = (0.0f64, 0.0f64, true);
    let mut placed = std::collections::HashSet::new();
    for (g, glyph) in page.glyphs.iter().enumerate() {
        if let Some(&t) = table_of.get(&g) {
            if placed.insert(t) { out.push_str(&format!("\n\n[Tabla T{:02} → tablas/T{:02}.csv]\n\n", t + 1, t + 1)); first = true }
            continue;
        }
        if !first && glyph.word_start {
            if (glyph.y - last_y).abs() > glyph.size * 1.5 { out.push('\n') }
            if glyph.x < last_end && (glyph.y - last_y).abs() > glyph.size * 0.5 { out.push('\n') }
            if glyph.x > last_end + glyph.size * 0.1 { out.push(' ') }
        } else if !first && (glyph.y - last_y).abs() > glyph.size * 0.5 { out.push('\n') }
        out.push_str(&glyph.text);
        first = false;
        last_y = glyph.y;
        last_end = glyph.x + glyph.advance;
    }
    out
}

/// The lines of a page (height in whole points, text without digits or
/// spaces) made of the glyphs `keep` picks.
fn line_keys(page: &Page, keep: impl Fn((usize, &Glyph)) -> bool) -> std::collections::HashSet<(i64, String)> {
    let mut by_line: std::collections::BTreeMap<i64, Vec<&Glyph>> = Default::default();
    for (g, glyph) in page.glyphs.iter().enumerate() {
        if glyph.text.trim().is_empty() || !keep((g, glyph)) { continue }
        by_line.entry(glyph.y.round() as i64).or_default().push(glyph);
    }
    by_line.into_iter().map(|(y, mut gs)| {
        gs.sort_by(|a, b| a.x.total_cmp(&b.x));
        (y, gs.iter().flat_map(|g| g.text.chars()).filter(|c| !c.is_whitespace() && !c.is_ascii_digit()).collect::<String>())
    }).filter(|(_, t)| !t.is_empty()).collect()
}

/// Below this many characters per page on average, the pages are images.
const SCANNED_CHARS_PER_PAGE: usize = 40;

pub fn extract(bytes: &[u8]) -> Result<Document, String> {
    let mut pdf = pdf_extract::Document::load_mem(bytes).map_err(|e| format!("PDF ilegible: {e}"))?;
    if pdf.is_encrypted() && pdf.decrypt("").is_err() { return Err("PDF protegido con contraseña".into()) }
    let page_ids = pdf.get_pages();
    let numbers: Vec<u32> = page_ids.keys().copied().collect();
    let count = numbers.len() as u32;
    if count == 0 { return Err("PDF sin páginas".into()) }
    let mut doc = Document { kind: "pdf", pages: Some(count), ..Default::default() };
    let mut collector = Collector::default();
    let mut unreadable = Vec::new();
    for n in &numbers {
        let before = collector.pages.len();
        if pdf_extract::output_doc_page(&pdf, &mut collector, *n).is_err() {
            collector.pages.truncate(before);
            collector.pages.push(Page::default());
            unreadable.push(*n);
        }
        let (horizontal, vertical, areas) = rules(&pdf, page_ids[n]);
        if let Some(page) = collector.pages.last_mut() { page.horizontal = horizontal; page.vertical = vertical; page.areas = areas; }
    }
    if !unreadable.is_empty() { doc.notes.push(format!("Páginas que no se pudieron leer: {unreadable:?}.")) }

    // Tables, page by page; one that goes on from the bottom of a page to the
    // top of the next with the same columns is the same table. Between the two
    // there may only be the running header and footer: lines at the same
    // height with the same text on both pages (the page number aside).
    let lines: Vec<std::collections::HashSet<(i64, String)>> = collector.pages.iter().map(|p| line_keys(p, |_| true)).collect();
    let mut pages_text = Vec::new();
    let mut last: Option<(usize, Vec<f64>, u32)> = None;
    for (i, page) in collector.pages.iter().enumerate() {
        let number = i as u32 + 1;
        let found = grids(page);
        let mut table_of = std::collections::HashMap::new();
        for (k, grid) in found.iter().enumerate() {
            let inside: std::collections::HashSet<usize> = grid.members.iter().copied().collect();
            let (top, bottom) = (grid.ys[0], *grid.ys.last().unwrap());
            let running = |keys: std::collections::HashSet<(i64, String)>, other: Option<&std::collections::HashSet<(i64, String)>>| keys.is_empty() || other.is_some_and(|o| keys.is_subset(o));
            let below_all = running(line_keys(page, |(g, gl)| !inside.contains(&g) && gl.y < bottom), lines.get(i + 1));
            let above_all = running(line_keys(page, |(g, gl)| !inside.contains(&g) && gl.y > top), i.checked_sub(1).and_then(|p| lines.get(p)));
            let same_columns = |xs: &Vec<f64>| xs.len() == grid.xs.len() && xs.iter().zip(&grid.xs).all(|(a, b)| (a - b).abs() <= SNAP * 2.0);
            let id = match &last {
                Some((t, xs, p)) if k == 0 && *p + 1 == number && above_all && same_columns(xs) => {
                    let mut rows = grid.rows.clone();
                    // A header row repeated on every page is written once.
                    if rows.first() == doc.tables[*t].rows.first() { rows.remove(0); }
                    doc.tables[*t].rows.extend(rows);
                    *t
                }
                _ => { doc.tables.push(Table { title: String::new(), page: Some(number), rows: grid.rows.clone() }); doc.tables.len() - 1 }
            };
            for &g in &grid.members { table_of.insert(g, id); }
            last = if k == found.len() - 1 && below_all { Some((id, grid.xs.clone(), number)) } else { None };
        }
        if found.is_empty() { last = None }
        pages_text.push(page_text(page, &table_of));
    }

    let chars: usize = pages_text.iter().map(|p| p.trim().chars().count()).sum();
    if chars < SCANNED_CHARS_PER_PAGE * pages_text.len() {
        doc.notes.push("El PDF casi no tiene texto (probablemente escaneado). DocPack no hace OCR: lee las imágenes de paginas/.".into());
    }
    if !doc.tables.is_empty() {
        doc.notes.push("Tablas detectadas por sus bordes dibujados; una tabla sin bordes queda como texto en su sección.".into());
    }
    let text_of = |from: u32, to: u32| -> String {
        (from..=to).map(|p| format!("<!-- página {p} -->\n{}\n", pages_text[(p - 1) as usize].trim())).collect::<Vec<_>>().join("\n")
    };
    // Bookmarks: (page, title), levels 1-2, merged when several start on one page.
    let mut marks: Vec<(u32, String, u8)> = Vec::new();
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
    marks.sort_by_key(|m| m.0);
    marks.dedup_by_key(|m| m.0);
    if marks.is_empty() {
        doc.notes.push("El PDF no tiene marcadores: una sección por página.".into());
        for p in 1..=count {
            let first = pages_text[(p - 1) as usize].lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("[Tabla")).unwrap_or("").chars().take(70).collect::<String>();
            let title = if first.is_empty() { format!("Página {p}") } else { format!("Página {p}: {first}") };
            doc.sections.push(Section { title, level: 1, page_start: Some(p), page_end: Some(p), text: text_of(p, p), ..Default::default() });
        }
    } else {
        if marks[0].0 > 1 {
            doc.sections.push(Section { title: "Inicio".into(), level: 1, page_start: Some(1), page_end: Some(marks[0].0 - 1), text: text_of(1, marks[0].0 - 1), ..Default::default() });
        }
        for (i, (page, title, level)) in marks.iter().enumerate() {
            let end = marks.get(i + 1).map(|m| m.0.saturating_sub(1).max(*page)).unwrap_or(count);
            doc.sections.push(Section { title: title.clone(), level: (*level).max(1), page_start: Some(*page), page_end: Some(end), text: text_of(*page, end), ..Default::default() });
        }
    }
    // Each table belongs to the section that holds its first page.
    for (t, table) in doc.tables.iter_mut().enumerate() {
        let page = table.page.unwrap_or(1);
        if let Some(section) = doc.sections.iter_mut().find(|s| s.page_start.unwrap_or(1) <= page && s.page_end.unwrap_or(count) >= page) {
            table.title = section.title.clone();
            section.tables.push(t);
        }
    }
    Ok(doc)
}

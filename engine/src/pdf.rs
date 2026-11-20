//! PDF: text page by page, read from where each character is drawn, and
//! tables from the lines the page draws around them. Sections come from the
//! document's own bookmarks (outline) when it has them, levels 1-2, each
//! covering the pages up to the next one; without bookmarks every page is a
//! section. Page markers stay in the text so any line can be cited by page.
//!
//! A table is a grid of ruling lines (borders) with text in at least two of
//! its cells, or, without borders, lines of text with blank lanes running down
//! through all of them (columns). Its characters go to the CSV and leave the
//! running text, which keeps a reference where the table was. Pages with
//! tables read by alignment, and pages whose text looks like columns that
//! could not be read as a table, are listed so the pack carries their image.
use crate::{color, CellColor, Document, Mark, Section, Table};
use std::collections::{HashMap, HashSet};
use lopdf::{content::Content, Dictionary, Object};
use pdf_extract::{ColorSpace, MediaBox, OutputDev, OutputError, Transform};

/// A drawn character: its baseline origin, advance and size, in page space,
/// the direction its line runs (the x axis of its text space) and the color
/// it is painted with ("FF0000"; None when the color space has no RGB).
#[derive(Debug, Clone)]
struct Glyph { x: f64, y: f64, advance: f64, size: f64, text: String, word_start: bool, dir: (f64, f64), color: Option<String> }

/// A horizontal (y, x0, x1) or vertical (x, y0, y1) ruling segment.
type Segment = (f64, f64, f64);

#[derive(Default)]
struct Page { glyphs: Vec<Glyph>, horizontal: Vec<Segment>, vertical: Vec<Segment>, areas: Vec<Area>, area_fills: Vec<Option<String>>, area: f64, image_area: f64, top: f64, bottom: f64, media: [f64; 4] }

/// A filled area (x0, y0, x1, y1): a cell's background, never a line by itself.
type Area = [f64; 4];

/// Receives the characters pdf-extract positions. The lines are read apart
/// (`Rules`): pdf-extract leaves even-odd fills (`f*`, how Word draws every
/// border) unpainted.
#[derive(Default)]
struct Collector { pages: Vec<Page>, word_start: bool, color: Option<String> }

/// "FF0000" of a color in a device (or calibrated, or ICC) color space, by
/// its number of components: gray, RGB or CMYK. None for spot colors and
/// patterns, whose RGB the page does not say.
fn device_hex(space: &ColorSpace, c: &[f64]) -> Option<String> {
    match space {
        ColorSpace::DeviceGray | ColorSpace::CalGray(_) | ColorSpace::DeviceRGB | ColorSpace::CalRGB(_) | ColorSpace::DeviceCMYK | ColorSpace::ICCBased(_) => components_hex(c),
        _ => None,
    }
}

/// "FF0000" of a color's components: none (the initial black), gray, RGB or CMYK.
fn components_hex(c: &[f64]) -> Option<String> {
    match c.len() {
        0 => Some(color::hex(0.0, 0.0, 0.0)),
        1 => Some(color::hex(c[0], c[0], c[0])),
        3 => Some(color::hex(c[0], c[1], c[2])),
        4 => { let k = 1.0 - c[3]; Some(color::hex((1.0 - c[0]) * k, (1.0 - c[1]) * k, (1.0 - c[2]) * k)) }
        _ => None,
    }
}

impl OutputDev for Collector {
    fn begin_page(&mut self, _n: u32, media: &MediaBox, _art: Option<(f64, f64, f64, f64)>) -> Result<(), OutputError> {
        self.pages.push(Page { area: ((media.urx - media.llx) * (media.ury - media.lly)).abs(), top: media.ury.max(media.lly), bottom: media.ury.min(media.lly), media: [media.llx.min(media.urx), media.lly.min(media.ury), media.llx.max(media.urx), media.lly.max(media.ury)], ..Default::default() });
        Ok(())
    }
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
        self.pages.last_mut().unwrap().glyphs.push(Glyph { x: trm.m31, y: trm.m32, advance: width * size + spacing * scale, size, text: text.to_string(), word_start, dir: (trm.m11, trm.m12), color: self.color.clone() });
        Ok(())
    }
    fn begin_word(&mut self) -> Result<(), OutputError> { self.word_start = true; Ok(()) }
    fn end_word(&mut self) -> Result<(), OutputError> { Ok(()) }
    fn end_line(&mut self) -> Result<(), OutputError> { Ok(()) }
    fn text_color(&mut self, space: &ColorSpace, c: &[f64]) -> Result<(), OutputError> { self.color = device_hex(space, c); Ok(()) }
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
struct Rules<'a> { pdf: &'a lopdf::Document, horizontal: Vec<Segment>, vertical: Vec<Segment>, areas: Vec<Area>, area_fills: Vec<Option<String>>, image_area: f64, fill: Option<String> }

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
                    else { self.areas.push([x0, y0, x1, y1]); self.area_fills.push(self.fill.clone()) }
                }
            }
        }
    }
    fn xobject(&self, resources: Option<&'a Dictionary>, name: &[u8]) -> Option<&'a lopdf::Stream> {
        let pdf: &'a lopdf::Document = self.pdf;
        let xobjects = resources?.get(b"XObject").ok().and_then(|x| pdf.dereference(x).ok())?.1.as_dict().ok()?;
        pdf.dereference(xobjects.get(name).ok()?).ok()?.1.as_stream().ok()
    }
    fn walk(&mut self, content: &[u8], resources: Option<&'a Dictionary>, start: Matrix, depth: u8) {
        let Ok(content) = Content::decode(content) else { return };
        let mut stack: Vec<(Matrix, Option<String>)> = Vec::new();
        let mut ctm = start;
        let mut pieces: Vec<Piece> = Vec::new();
        let (mut at, mut first) = ((0.0, 0.0), (0.0, 0.0));
        for op in &content.operations {
            let n: Vec<f64> = op.operands.iter().map(number).collect();
            match op.operator.as_str() {
                "q" => stack.push((ctm, self.fill.clone())),
                "Q" => { let (m, f) = stack.pop().unwrap_or((start, None)); ctm = m; self.fill = f }
                // The fill color areas are painted with, by its components.
                "g" | "rg" | "k" | "sc" | "scn" => {
                    let numeric = op.operands.iter().all(|o| o.as_float().is_ok() || o.as_i64().is_ok());
                    self.fill = if numeric && !n.is_empty() { components_hex(&n) } else { None };
                }
                "cs" => self.fill = Some(color::hex(0.0, 0.0, 0.0)),
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
                    let Some(stream) = self.xobject(resources, name) else { continue };
                    let subtype = stream.dict.get(b"Subtype").and_then(|s| s.as_name()).ok();
                    // An image fills the unit square: its area on the page is the
                    // matrix's determinant. A scanned page is mostly one image.
                    if subtype == Some(b"Image".as_slice()) { self.image_area += (ctm[0] * ctm[3] - ctm[1] * ctm[2]).abs(); continue }
                    if subtype != Some(b"Form".as_slice()) { continue }
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

/// What a page draws besides its text: ruling lines, filled areas, images.
fn draw(pdf: &lopdf::Document, id: lopdf::ObjectId, page: &mut Page) {
    let mut rules = Rules { pdf, horizontal: Vec::new(), vertical: Vec::new(), areas: Vec::new(), area_fills: Vec::new(), image_area: 0.0, fill: Some(color::hex(0.0, 0.0, 0.0)) };
    let Ok(content) = pdf.get_page_content(id) else { return };
    // A page's resources are its own dictionary or one it refers to (also
    // inherited from its parents).
    let resources = pdf.get_page_resources(id).ok().and_then(|(own, refs)| own.or_else(|| refs.iter().find_map(|r| pdf.get_dictionary(*r).ok())));
    rules.walk(&content, resources, IDENTITY, 0);
    (page.horizontal, page.vertical, page.areas, page.area_fills, page.image_area) = (rules.horizontal, rules.vertical, rules.areas, rules.area_fills, rules.image_area);
}

/// A page whose text runs up, down or upside down (a landscape report on a
/// page turned with /Rotate, a table drawn sideways) is turned a quarter turn
/// at a time until most of its letters read left to right: letters, rules and
/// areas alike, so its lines, columns and running header are read as on
/// screen. Which turn comes from the letters themselves, not from /Rotate,
/// which some writers set on pages whose text already reads across.
fn upright(page: &mut Page) {
    let mut votes = [0usize; 4];
    for g in page.glyphs.iter().filter(|g| !g.text.trim().is_empty()) {
        let (dx, dy) = g.dir;
        votes[if dx.abs() >= dy.abs() { if dx >= 0.0 { 0 } else { 2 } } else if dy > 0.0 { 1 } else { 3 }] += 1;
    }
    let turn = (0..4).max_by_key(|&q| votes[q]).unwrap_or(0);
    if turn == 0 || votes[turn] * 2 <= votes.iter().sum::<usize>() { return }
    let [x0, y0, x1, y1] = page.media;
    // 1: lines run up, 2: upside down, 3: lines run down; each to the right.
    let to = |x: f64, y: f64| -> (f64, f64) {
        match turn { 1 => (y - y0, x1 - x), 2 => (x1 - x, y1 - y), _ => (y1 - y, x - x0) }
    };
    let (width, height) = if turn == 2 { (x1 - x0, y1 - y0) } else { (y1 - y0, x1 - x0) };
    for g in &mut page.glyphs {
        (g.x, g.y) = to(g.x, g.y);
        let (dx, dy) = g.dir;
        g.dir = match turn { 1 => (dy, -dx), 2 => (-dx, -dy), _ => (-dy, dx) };
    }
    let mut ends: Vec<((f64, f64), (f64, f64))> = page.horizontal.iter().map(|&(y, a, b)| ((a, y), (b, y))).collect();
    ends.extend(page.vertical.iter().map(|&(x, a, b)| ((x, a), (x, b))));
    let (mut horizontal, mut vertical) = (Vec::new(), Vec::new());
    for ((ax, ay), (bx, by)) in ends {
        let ((ax, ay), (bx, by)) = (to(ax, ay), to(bx, by));
        if (ay - by).abs() <= SNAP { horizontal.push(((ay + by) / 2.0, ax.min(bx), ax.max(bx))) } else { vertical.push(((ax + bx) / 2.0, ay.min(by), ay.max(by))) }
    }
    (page.horizontal, page.vertical) = (horizontal, vertical);
    for a in &mut page.areas {
        let ((ax, ay), (bx, by)) = (to(a[0], a[1]), to(a[2], a[3]));
        *a = [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)];
    }
    page.media = [0.0, 0.0, width, height];
    (page.top, page.bottom) = (height, 0.0);
}

/// Points within which two lines are the same line (a border drawn twice,
/// half a point of rounding between writers).
const SNAP: f64 = 1.5;
/// A filled rectangle at most this thick (points) is a drawn border.
const THIN: f64 = 3.0;

/// A table found on a page: its column lines (with how far each may move and
/// still be the same column), top and bottom, cells and glyphs. A table read
/// by its borders has fixed columns; one read by its aligned text has columns
/// as wide as the blank lane between them.
struct Grid { xs: Vec<f64>, slack: Vec<f64>, top: f64, bottom: f64, rows: Vec<Vec<String>>, members: Vec<usize>, stream: bool,
    /// The glyphs of each cell, row by row as in `rows`: their colors.
    glyphs: Vec<Vec<Vec<usize>>> }

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
        // Its columns: the vertical lines, and the ends of the horizontal ones
        // (a table open at the sides draws no border there).
        let ends = [hs.iter().map(|&i| h[i].1).fold(f64::MAX, f64::min), hs.iter().map(|&i| h[i].2).fold(f64::MIN, f64::max)];
        let xs = cluster(vs.iter().map(|&j| v[j].0).chain(ends).collect());
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
        let all: Vec<Vec<String>> = cells.iter().enumerate().map(|(r, row)| row.iter().enumerate().map(|(c, glyphs)| {
            let right = (c + 1..xs.len()).find(|&k| k == xs.len() - 1 || has_wall(xs[k], ys[r], ys[r + 1])).map(|k| xs[k]).unwrap_or(xs[c + 1]);
            cell_text(page, glyphs, xs[c], right)
        }).collect::<Vec<_>>()).collect();
        let keep: Vec<bool> = all.iter().map(|r| r.iter().any(|c| !c.is_empty())).collect();
        let rows: Vec<Vec<String>> = all.into_iter().zip(&keep).filter(|(_, k)| **k).map(|(r, _)| r).collect();
        let glyphs: Vec<Vec<Vec<usize>>> = cells.into_iter().zip(&keep).filter(|(_, k)| **k).map(|(r, _)| r).collect();
        out.push(Grid { slack: vec![0.0; xs.len()], top: ys[0], bottom: *ys.last().unwrap(), xs, rows, members, stream: false, glyphs });
    }
    out
}

/// A line of text on a page: its baseline, largest letter, glyphs, and the x
/// ranges its words cover (glyphs closer than a blank lane joined).
struct Line { y: f64, size: f64, glyphs: Vec<usize>, chunks: Vec<(f64, f64)> }

/// Fraction of the letter size a blank lane between columns is at least
/// wide: more than a space between words.
const LANE: f64 = 0.6;

/// A glyph whose line runs left to right on the page as read (after
/// `upright`). Letters running another way (a footer set sideways, a label
/// up a margin) are read apart, in their own direction.
fn across(g: &Glyph) -> bool { g.dir.0 > 0.0 && g.dir.1.abs() <= g.dir.0 * 0.1 }

/// The text of the letters that run another way than the page (`across` is
/// false), read in their own direction: a line per baseline, its words in
/// order. Each direction is a block of its own.
fn other_ways(page: &Page, skip: &HashSet<usize>) -> String {
    let mut by_way: std::collections::BTreeMap<u8, Vec<(f64, f64, &Glyph)>> = Default::default();
    for (g, glyph) in page.glyphs.iter().enumerate() {
        if skip.contains(&g) || across(glyph) { continue }
        let (dx, dy) = glyph.dir;
        // Turned so the glyph's line runs to the right: up, upside down, down.
        let (way, (x, y)) = if dx.abs() >= dy.abs() { (2, (-glyph.x, -glyph.y)) } else if dy > 0.0 { (1, (glyph.y, -glyph.x)) } else { (3, (-glyph.y, glyph.x)) };
        by_way.entry(way).or_default().push((x, y, glyph));
    }
    let mut out = Vec::new();
    for mut gs in by_way.into_values() {
        gs.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut lines: Vec<Vec<(f64, f64, &Glyph)>> = Vec::new();
        for g in gs {
            match lines.last_mut() { Some(l) if (l[0].1 - g.1).abs() <= g.2.size.max(1.0) * 0.5 => l.push(g), _ => lines.push(vec![g]) }
        }
        let text: Vec<String> = lines.into_iter().map(|mut l| {
            l.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (mut t, mut end) = (String::new(), f64::MIN);
            for (x, _, g) in l {
                if !t.is_empty() && x > end + g.size * 0.1 && !t.ends_with(' ') { t.push(' ') }
                t.push_str(&g.text);
                if !g.text.trim().is_empty() { end = x + g.advance }
            }
            t.split_whitespace().collect::<Vec<_>>().join(" ")
        }).filter(|t| !t.is_empty()).collect();
        if !text.is_empty() { out.push(text.join("
")) }
    }
    out.join("

")
}

/// The lines made of the glyphs no table took, top to bottom.
fn text_lines(page: &Page, taken: &HashSet<usize>) -> Vec<Line> {
    let mut order: Vec<usize> = (0..page.glyphs.len()).filter(|&g| !taken.contains(&g) && across(&page.glyphs[g])).collect();
    order.sort_by(|&a, &b| page.glyphs[b].y.total_cmp(&page.glyphs[a].y));
    let mut lines: Vec<Line> = Vec::new();
    for g in order {
        let glyph = &page.glyphs[g];
        let size = glyph.size.max(1.0);
        match lines.last_mut() {
            Some(l) if (l.y - glyph.y).abs() <= l.size.max(size) * 0.5 => { l.glyphs.push(g); l.size = l.size.max(size) }
            _ => lines.push(Line { y: glyph.y, size, glyphs: vec![g], chunks: Vec::new() }),
        }
    }
    for l in &mut lines {
        let mut spans: Vec<(f64, f64, &str)> = l.glyphs.iter().map(|&g| &page.glyphs[g]).filter(|g| !g.text.trim().is_empty()).map(|g| (g.x, g.x + g.advance.max(0.0), g.text.as_str())).collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        // (start, end, text) of each chunk: a bullet or a dash is joined to
        // the words it marks, which follow it within a few letters: a list
        // inside a cell is indented text, not a column of its own.
        let mut chunks: Vec<(f64, f64, String)> = Vec::new();
        for (a, b, t) in spans {
            match chunks.last_mut() {
                Some(c) if a - c.1 < l.size * LANE || (bullet(&c.2) && a - c.1 < l.size * 2.5) => { c.1 = c.1.max(b); c.2.push_str(t) }
                _ => chunks.push((a, b, t.to_string())),
            }
        }
        l.chunks = chunks.into_iter().map(|(a, b, _)| (a, b)).collect();
    }
    lines.retain(|l| !l.chunks.is_empty());
    lines
}

/// The blank lanes down through `lines`: x ranges between their words that no
/// glyph of any of them touches, wider than a space between words.
fn lanes(lines: &[Line]) -> Vec<(f64, f64)> {
    let mut chunks: Vec<(f64, f64)> = lines.iter().flat_map(|l| l.chunks.iter().copied()).collect();
    chunks.sort_by(|a, b| a.0.total_cmp(&b.0));
    let size = lines.iter().map(|l| l.size).sum::<f64>() / lines.len().max(1) as f64;
    let mut out = Vec::new();
    let mut end: Option<f64> = None;
    for (a, b) in chunks {
        if let Some(e) = end { if a - e >= size * LANE { out.push((e, a)) } }
        end = Some(end.map_or(b, |e| e.max(b)));
    }
    out
}

/// The column (from 0) of `x` between `lanes`.
fn column(x: f64, lanes: &[(f64, f64)]) -> usize { lanes.iter().filter(|l| l.1 <= x).count() }

/// How many columns between `lanes` a line has words in.
fn columns_used(line: &Line, lanes: &[(f64, f64)]) -> usize {
    let mut used: Vec<usize> = line.chunks.iter().map(|c| column((c.0 + c.1) / 2.0, lanes)).collect();
    used.dedup();
    used.len()
}

/// The words of a line from `x0` to `x1`, a space where they are apart.
fn line_text(page: &Page, line: &Line, x0: f64, x1: f64) -> String {
    let mut gs: Vec<&Glyph> = line.glyphs.iter().map(|&g| &page.glyphs[g]).filter(|g| { let c = g.x + g.advance / 2.0; c >= x0 && c <= x1 }).collect();
    gs.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut text = String::new();
    let mut end = f64::MIN;
    for g in gs {
        if !text.is_empty() && g.x > end + g.size * 0.1 && !text.ends_with(' ') { text.push(' ') }
        text.push_str(&g.text);
        if !g.text.trim().is_empty() { end = g.x + g.advance }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The distance between two lines of one paragraph or cell in `block`: the
/// smallest that repeats (rows are spaced wider than the lines of a cell).
/// Lines closer than their letters are tall are words of two columns at
/// different heights, not two lines of text.
fn line_spacing(block: &[Line]) -> f64 {
    let gaps: Vec<f64> = block.windows(2).filter(|w| w[0].y - w[1].y >= w[0].size.min(w[1].size) * 0.9).map(|w| w[0].y - w[1].y).collect();
    let mut sorted = gaps.clone();
    sorted.sort_by(f64::total_cmp);
    sorted.iter().copied().find(|&g| gaps.iter().filter(|&&o| (o - g).abs() <= 0.5).count() >= 2)
        .or_else(|| sorted.get(sorted.len() / 2).copied())
        .unwrap_or_else(|| block.first().map_or(12.0, |l| l.size * 1.2))
}

/// Tables without borders: consecutive lines with at least two blank lanes
/// running down through all of them (three columns or more). A paragraph
/// closes every lane within a line or two; columns keep theirs open. Also says
/// whether the page has text in columns that was not read as a table: lanes
/// through lines too few or too empty to be rows, or two columns with words on
/// both sides that are not a list (its bullets or numbers on the left).
fn streams(page: &Page, taken: &HashSet<usize>) -> (Vec<Grid>, bool) {
    let lines = text_lines(page, taken);
    // Lines of one table are at most four letters apart (a shaded header
    // stands further from its first row than lines of text do) and of one
    // letter size: a heading or a paragraph in letters of another size, or the
    // caption of the next table, starts something else. Without that, short
    // lines between two tables keep the lanes of both open and the first table
    // takes the second.
    let captions: Vec<bool> = lines.iter().map(|l| caption(&line_text(page, l, f64::MIN, f64::MAX))).collect();
    let near = |a: &Line, b: &Line| a.y - b.y <= 4.0 * a.size.max(b.size) && (a.size - b.size).abs() <= a.size.min(b.size) * 0.1;
    // From line `i`, the lines before `end` that keep at least `least` lanes open.
    let grow = |i: usize, end: usize, least: usize| {
        let mut j = i + 1;
        while j < end && near(&lines[j - 1], &lines[j]) && !captions[j] && lanes(&lines[i..=j]).len() >= least { j += 1 }
        j
    };
    let (mut out, mut doubtful) = (Vec::new(), false);
    // Three columns or more. `floor`: the first line after the last table.
    let mut used: Vec<(usize, usize)> = Vec::new();
    let (mut i, mut floor) = (0, 0);
    while i < lines.len() {
        let seed = i;
        let mut j = grow(i, lines.len(), 2);
        // A line with words in one column only that closes a lane is the end
        // of the paragraph above the table, or the start of the one below.
        loop {
            if j - i <= 2 { break }
            let open = lanes(&lines[i..j]).len();
            let rest = lanes(&lines[i + 1..j]);
            if rest.len() > open && columns_used(&lines[i], &rest) <= 1 { i += 1; j = grow(i, lines.len(), 2); continue }
            let rest = lanes(&lines[i..j - 1]);
            if rest.len() > open && columns_used(&lines[j - 1], &rest) <= 1 { j -= 1; continue }
            break;
        }
        let open = if j - i >= 2 { lanes(&lines[i..j]) } else { Vec::new() };
        if open.len() >= 2 {
            // Lines just above at the table's own spacing that keep every lane
            // open belong to it (header cells that start higher than others).
            let spacing = line_spacing(&lines[i..j]);
            while i > floor && !captions[i] && near(&lines[i - 1], &lines[i]) && lines[i - 1].y - lines[i].y <= spacing * 1.3 && lanes(&lines[i - 1..j]).len() == open.len() { i -= 1 }
            match stream_table(page, &lines[i..j], &lanes(&lines[i..j]), false) {
                Some(grid) => { out.push(grid); used.push((i, j)); i = j; floor = j; continue }
                None => doubtful |= j - i >= 3,
            }
        }
        i = seed + 1;
    }
    // Two columns, in the lines no table took: a table when rules or shaded
    // rows divide it, else perhaps one.
    let mut free: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;
    for &(a, b) in &used { if a > start { free.push((start, a)) } start = b }
    if start < lines.len() { free.push((start, lines.len())) }
    for (a, b) in free {
        let mut i = a;
        while i < b {
            let k = grow(i, b, 1);
            // Two lines are enough when rules close them (a header and one row
            // at the foot of a page); unruled, three lines are the least that
            // may only look like a table.
            if k - i < 2 { i += 1; continue }
            match stream_table(page, &lines[i..k], &lanes(&lines[i..k]), true) {
                Some(grid) => out.push(grid),
                None if k - i < 3 => { i += 1; continue }
                None => doubtful = doubtful || two_columns(page, &lines[i..k]),
            }
            i = k;
        }
    }
    (out, doubtful)
}

/// A list's bullet or dash alone ("•", "–", "-", "▪", "o"): no letter or
/// digit, so a column of numbers is never one.
fn bullet(text: &str) -> bool {
    let t = text.trim();
    !t.is_empty() && t.chars().count() <= 2 && (t == "o" || !t.chars().any(char::is_alphanumeric))
}

/// A caption: a word for table or figure and its number at the start of a
/// line, the number ending in ":" or "." or followed by a dash ("Table 12:",
/// "Table 8–34: …", "Tabla 3. …", "Figure 7 – …").
fn caption(text: &str) -> bool {
    let mut words = text.split_whitespace();
    let (Some(word), Some(number)) = (words.next(), words.next()) else { return false };
    let named = ["table", "tabla", "tableau", "figure", "figura", "cuadro", "exhibit"].contains(&word.to_lowercase().as_str());
    let numbered = number.starts_with(|c: char| c.is_ascii_digit());
    named && numbered && (number.ends_with([':', '.']) || words.next().is_some_and(|w| matches!(w, ":" | "–" | "—" | "-")))
}

/// A figure's caption ("Figure 3: …"): a caption, but of no table.
fn figure(text: &str) -> bool {
    text.split_whitespace().next().is_some_and(|w| ["figure", "figura", "exhibit"].contains(&w.to_lowercase().as_str()))
}

/// A list's mark: a bullet ("•", "o", "–"), a short number or letter ending
/// in "." or ")", or a heading's number ("8.2.5.1").
fn list_mark(text: &str) -> bool {
    let t = text.trim();
    let short = t.chars().count() <= 3 && (!t.chars().any(char::is_alphanumeric) || t == "o" || (t.ends_with(['.', ')']) && t.chars().count() > 1));
    short || (t.contains('.') && t.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

/// Lines with one lane open down through them look like a two-column table
/// when at least three have words on both sides and the left words are not
/// only list marks (a bullet, "1.", "a)").
fn two_columns(page: &Page, block: &[Line]) -> bool {
    let open = lanes(block);
    let Some(&(left_end, right_start)) = open.first() else { return false };
    let both: Vec<String> = block.iter().filter(|l| columns_used(l, &open) == 2).map(|l| line_text(page, l, f64::MIN, left_end)).collect();
    both.len() >= 3 && !both.iter().all(|t| list_mark(t)) && right_start > left_end
}

/// The table in `block` with the columns between `lanes`, when at least two of
/// its rows have words in two columns or more. A row is one line, or several
/// when the table spaces its rows wider than its lines (cells of several
/// lines); with even spacing, a line with nothing in the first column goes on
/// the row above. A rule or the edge of a shaded area between two lines (a
/// header's background) always starts a row, and lines in one column above
/// the first of them are the table's caption, not a row. When two or more
/// rules divide the body, they alone make the rows. `ruled_only`: read it
/// only then (two columns of text are a table only when ruled).
fn stream_table(page: &Page, block: &[Line], lanes: &[(f64, f64)], ruled_only: bool) -> Option<Grid> {
    let columns = lanes.len() + 1;
    let left = block.iter().map(|l| l.chunks[0].0).fold(f64::MAX, f64::min);
    let right = block.iter().map(|l| l.chunks.last().unwrap().1).fold(f64::MIN, f64::max);
    let gaps: Vec<f64> = block.windows(2).map(|w| w[0].y - w[1].y).collect();
    let wide = line_spacing(block) * 1.15;
    let by_space = gaps.iter().filter(|&&g| g > wide).count() >= 2;
    let between = |e: f64, above: &Line, below: &Line| e < above.y - above.size * 0.2 && e > below.y + below.size * 0.8;
    let edge_between = |above: &Line, below: &Line| page.areas.iter().any(|a| a[0] < right && a[2] > left && (between(a[1], above, below) || between(a[3], above, below)))
        || page.horizontal.iter().any(|h| h.1 < right && h.2 > left && between(h.0, above, below));
    // Lines in one column above the first rule are the caption; below it,
    // rules that divide the body (two or more) are the rows, else spacing.
    let edges: Vec<bool> = (0..block.len()).map(|k| k > 0 && edge_between(&block[k - 1], &block[k])).collect();
    let mut from = 0;
    if let Some(first) = edges.iter().position(|&e| e) {
        if block[..first].iter().all(|l| columns_used(l, lanes) <= 1) { from = first }
    }
    // The rule that closes the table under its last line divides rows too: a
    // header and one row of two lines have one rule between them and one below.
    let last = block.last().unwrap();
    let closed = page.horizontal.iter().any(|h| h.1 < right && h.2 > left && h.0 < last.y && h.0 > last.y - last.size * 2.0);
    let ruled = edges[from + 1..].iter().filter(|&&e| e).count() + usize::from(closed && edges[from + 1..].iter().any(|&e| e));
    if ruled_only && ruled < 2 { return None }
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for (k, line) in block.iter().enumerate().skip(from) {
        let starts = k == from || edges[k] || (ruled < 2 && (gaps[k - 1] > wide
            || (!by_space && column((line.chunks[0].0 + line.chunks[0].1) / 2.0, lanes) == 0)));
        if starts { rows.push(Vec::new()) }
        rows.last_mut().unwrap().push(k);
    }
    // A last row of one line in one column, further from the table than its
    // rows are from each other, is the heading that follows it.
    if rows.len() > 2 {
        let above = |r: &Vec<usize>| block[r[0] - 1].y - block[r[0]].y;
        let last = rows.last().unwrap();
        let widest = rows[1..rows.len() - 1].iter().map(above).fold(0.0, f64::max);
        if last.len() == 1 && columns_used(&block[last[0]], lanes) == 1 && above(last) > widest * 1.2 { rows.pop(); }
    }
    let walls: Vec<(f64, f64)> = (0..columns).map(|c| (if c == 0 { left } else { lanes[c - 1].1 }, if c + 1 == columns { right + block[0].size } else { lanes[c].1 })).collect();
    let mut members = Vec::new();
    let glyphs: Vec<Vec<Vec<usize>>> = rows.iter().map(|row| {
        let mut by_column: Vec<Vec<usize>> = vec![Vec::new(); columns];
        for &k in row {
            for &g in &block[k].glyphs {
                let glyph = &page.glyphs[g];
                by_column[column(glyph.x + glyph.advance / 2.0, lanes).min(columns - 1)].push(g);
                members.push(g);
            }
        }
        by_column
    }).collect();
    let cells: Vec<Vec<String>> = glyphs.iter().map(|by_column| by_column.iter().enumerate().map(|(c, gs)| cell_text(page, gs, walls[c].0, walls[c].1)).collect()).collect();
    if cells.iter().filter(|r| r.iter().filter(|c| !c.is_empty()).count() >= 2).count() < 2 { return None }
    // A column of nothing but bullets, its first row too, is a list.
    if (0..columns).any(|c| { let mut col = cells.iter().map(|r| &r[c]).filter(|t| !t.is_empty()).peekable(); col.peek().is_some() && col.all(|t| t.lines().all(list_mark)) }) { return None }
    let mut xs = vec![left];
    xs.extend(lanes.iter().map(|l| (l.0 + l.1) / 2.0));
    xs.push(right);
    let mut slack = vec![f64::INFINITY];
    slack.extend(lanes.iter().map(|l| (l.1 - l.0) / 2.0));
    slack.push(f64::INFINITY);
    let (first, last) = (&block[rows[0][0]], &block[*rows.last()?.last()?]);
    Some(Grid { xs, slack, top: first.y + first.size, bottom: last.y - last.size * 0.3, rows: cells, members, stream: true, glyphs })
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
    // first letter of the next line, which starts where this one does. Then
    // the two lines are one word.
    let padding = lines.iter().map(|l| l.2 - left).fold(f64::MAX, f64::min).max(0.0);
    let mut out = String::new();
    for (i, (text, end, ..)) in lines.iter().enumerate() {
        out.push_str(&text.split_whitespace().collect::<Vec<_>>().join(" "));
        if let Some(next) = lines.get(i + 1) {
            // Centred lines start at different places: never one split word.
            let full = right - padding - end < next.3 && (next.2 - lines[i].2).abs() <= 1.0;
            if text.ends_with(char::is_whitespace) || !full { out.push('\n') }
        }
    }
    out
}

/// The pieces of a cell's text in a color that marks them (not the body's),
/// in reading order: consecutive letters of one color are one piece.
fn color_pieces(page: &Page, glyphs: &[usize], body: &str) -> Vec<Mark> {
    let mut gs: Vec<&Glyph> = glyphs.iter().map(|&g| &page.glyphs[g]).collect();
    gs.sort_by(|a, b| b.y.total_cmp(&a.y));
    let mut lines: Vec<Vec<&Glyph>> = Vec::new();
    for g in gs {
        match lines.last_mut() { Some(l) if (l[0].y - g.y).abs() <= g.size * 0.5 => l.push(g), _ => lines.push(vec![g]) }
    }
    let mut out: Vec<Mark> = Vec::new();
    let mut open: Option<(String, String, f64)> = None; // (color, text, end x)
    for line in &mut lines {
        line.sort_by(|a, b| a.x.total_cmp(&b.x));
        for g in line.iter() {
            if g.text.trim().is_empty() { if let Some(o) = open.as_mut() { if !o.1.ends_with(' ') { o.1.push(' ') } } continue }
            let mark = g.color.clone().filter(|c| color::marks_text(c, body));
            match (&mut open, mark) {
                (Some(o), Some(c)) if o.0 == c => {
                    if g.x > o.2 + g.size * 0.1 && !o.1.ends_with(' ') { o.1.push(' ') }
                    o.1.push_str(&g.text);
                    o.2 = g.x + g.advance;
                }
                (_, Some(c)) => {
                    if let Some((c0, t, _)) = open.take() { out.push(Mark { text: t.trim().to_string(), color: Some(c0), fill: None }) }
                    open = Some((c, g.text.clone(), g.x + g.advance));
                }
                (_, None) => if let Some((c0, t, _)) = open.take() { out.push(Mark { text: t.trim().to_string(), color: Some(c0), fill: None }) },
            }
        }
        if let Some(o) = open.as_mut() { if !o.1.ends_with(' ') { o.1.push(' ') } }
    }
    if let Some((c0, t, _)) = open.take() { out.push(Mark { text: t.trim().to_string(), color: Some(c0), fill: None }) }
    out.retain(|m| !m.text.is_empty());
    out
}

/// The fill behind a cell's text: the smallest filled area under its middle,
/// when its color marks (not white).
fn cell_fill(page: &Page, glyphs: &[usize]) -> Option<String> {
    let visible: Vec<&Glyph> = glyphs.iter().map(|&g| &page.glyphs[g]).filter(|g| !g.text.trim().is_empty()).collect();
    if visible.is_empty() { return None }
    let x = visible.iter().map(|g| g.x + g.advance / 2.0).sum::<f64>() / visible.len() as f64;
    let y = visible.iter().map(|g| g.y + g.size * 0.3).sum::<f64>() / visible.len() as f64;
    page.areas.iter().zip(&page.area_fills)
        .filter(|(a, _)| a[0] <= x && a[2] >= x && a[1] <= y && a[3] >= y)
        .min_by(|(a, _), (b, _)| ((a[2] - a[0]) * (a[3] - a[1])).total_cmp(&((b[2] - b[0]) * (b[3] - b[1]))))
        .and_then(|(_, f)| f.clone()).filter(|f| color::marks_fill(f))
}

/// The colored cells of a grid, rows from `skip` on, numbered from `base`.
fn grid_colors(page: &Page, grid: &Grid, body: &str, skip: usize, base: usize) -> Vec<CellColor> {
    let mut out = Vec::new();
    for (r, row) in grid.glyphs.iter().enumerate().skip(skip) {
        for (c, glyphs) in row.iter().enumerate() {
            let (fill, text) = (cell_fill(page, glyphs), color_pieces(page, glyphs, body));
            if fill.is_some() || !text.is_empty() { out.push(CellColor { row: base + r - skip, column: c, fill, text }) }
        }
    }
    out
}

/// The glyphs of running text in a color that marks them, with their color:
/// only where the color covers part of its line (a line all in one color is
/// the document's style for a heading or a caption), and only a run of two
/// letters or more (a lone colored glyph is a bullet drawn in a symbol font).
fn text_marks(page: &Page, skip: &HashSet<usize>, body: &str) -> HashMap<usize, String> {
    let mut by_line: std::collections::BTreeMap<i64, Vec<usize>> = Default::default();
    for (g, glyph) in page.glyphs.iter().enumerate() {
        if !skip.contains(&g) && !glyph.text.trim().is_empty() { by_line.entry(glyph.y.round() as i64).or_default().push(g) }
    }
    let mut out = HashMap::new();
    for gs in by_line.values() {
        let mut gs = gs.clone();
        gs.sort_by(|&a, &b| page.glyphs[a].x.total_cmp(&page.glyphs[b].x));
        let color_of = |g: usize| page.glyphs[g].color.clone().filter(|c| color::marks_text(c, body));
        let marked = gs.iter().filter(|&&g| color_of(g).is_some()).count();
        if marked == 0 || marked == gs.len() { continue }
        // Runs of one color, left to right.
        let mut run: Vec<usize> = Vec::new();
        let flush = |run: &mut Vec<usize>, out: &mut HashMap<usize, String>| {
            if run.len() >= 2 { if let Some(c) = color_of(run[0]) { for &g in run.iter() { out.insert(g, c.clone()); } } }
            run.clear();
        };
        for &g in &gs {
            match (color_of(g), run.first().and_then(|&f| color_of(f))) {
                (Some(c), Some(r)) if c == r => run.push(g),
                (Some(_), _) => { flush(&mut run, &mut out); run.push(g) }
                (None, _) => flush(&mut run, &mut out),
            }
        }
        flush(&mut run, &mut out);
    }
    out
}

/// Running text of a page in drawing order (the order pdf-extract's own
/// plain text uses), with the tables' characters replaced by a reference and
/// the `hidden` ones (the running header and footer) left out.
fn page_text(page: &Page, table_of: &HashMap<usize, usize>, hidden: &HashSet<usize>, marks: &HashMap<usize, String>) -> (String, Vec<(String, String)>) {
    let mut out = String::new();
    let (mut last_y, mut last_end, mut first) = (0.0f64, 0.0f64, true);
    let mut placed = HashSet::new();
    // The colored piece being written: its color, where its text starts.
    let mut open: Option<(String, usize)> = None;
    let mut pieces: Vec<(String, String)> = Vec::new();
    let close = |out: &mut String, open: &mut Option<(String, usize)>, pieces: &mut Vec<(String, String)>| {
        if let Some((c, at)) = open.take() {
            let text = out[at..].to_string();
            out.truncate(at);
            let trail = text.len() - text.trim_end().len();
            out.push_str(&color::wrap(text.trim_end(), Some(&c), None));
            out.push_str(&" ".repeat(trail));
            pieces.push((c, text.trim().to_string()));
        }
    };
    for (g, glyph) in page.glyphs.iter().enumerate() {
        if hidden.contains(&g) || !across(glyph) { continue }
        if let Some(&t) = table_of.get(&g) {
            close(&mut out, &mut open, &mut pieces);
            if placed.insert(t) { out.push_str(&format!("\n\n[Tabla T{:02} → tablas/T{:02}.csv]\n\n", t + 1, t + 1)); first = true }
            continue;
        }
        let mut sep = String::new();
        if !first && glyph.word_start {
            if (glyph.y - last_y).abs() > glyph.size * 1.5 { sep.push('\n') }
            if glyph.x < last_end && (glyph.y - last_y).abs() > glyph.size * 0.5 { sep.push('\n') }
            if glyph.x > last_end + glyph.size * 0.1 { sep.push(' ') }
        } else if !first && (glyph.y - last_y).abs() > glyph.size * 0.5 { sep.push('\n') }
        let mark = marks.get(&g);
        let blank = glyph.text.trim().is_empty();
        // A piece ends at a line's end or where the color changes.
        if sep.contains('\n') || (!blank && open.as_ref().map(|o| &o.0) != mark) { close(&mut out, &mut open, &mut pieces) }
        out.push_str(&sep);
        if let (Some(c), None, false) = (mark, &open, blank) { open = Some((c.clone(), out.len())) }
        out.push_str(&glyph.text);
        first = false;
        last_y = glyph.y;
        last_end = glyph.x + glyph.advance;
    }
    close(&mut out, &mut open, &mut pieces);
    // Letters running another way, after the page's own text.
    let skip: HashSet<usize> = hidden.iter().copied().chain(table_of.keys().copied()).collect();
    let other = other_ways(page, &skip);
    if !other.is_empty() { out.push_str("

"); out.push_str(&other) }
    (out, pieces)
}

/// The lines of a page (height in whole points, text without digits or
/// spaces) made of the glyphs `keep` picks.
fn line_keys(page: &Page, keep: impl Fn((usize, &Glyph)) -> bool) -> HashSet<(i64, String)> {
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

/// The running header and footer of a page: the lines in its top and bottom
/// margins (an eighth of the page each) that the page before or after has at
/// the same height with the same words (numbers and short words aside: "ii",
/// "74"), and lines of a page number alone. Their glyphs, which no table
/// without borders takes.
fn running_band(page: &Page, neighbors: &[&HashSet<(i64, String)>]) -> HashSet<usize> {
    let mut by_line: std::collections::BTreeMap<i64, Vec<usize>> = Default::default();
    for (g, glyph) in page.glyphs.iter().enumerate() { by_line.entry(glyph.y.round() as i64).or_default().push(g) }
    let lines: Vec<(i64, Vec<usize>)> = by_line.into_iter().rev().filter(|(_, gs)| gs.iter().any(|&g| !page.glyphs[g].text.trim().is_empty())).collect();
    let margin = (page.top - page.bottom) / 8.0;
    let running = |(y, gs): &(i64, Vec<usize>)| {
        if (*y as f64) < page.top - margin && (*y as f64) > page.bottom + margin { return false }
        let mut gs: Vec<&Glyph> = gs.iter().map(|&g| &page.glyphs[g]).collect();
        gs.sort_by(|a, b| a.x.total_cmp(&b.x));
        let text: String = gs.iter().map(|g| g.text.as_str()).collect();
        let key = band_key(&text);
        if key.is_empty() { return page_number(&text) }
        neighbors.iter().any(|n| n.contains(&(*y, key.clone())))
    };
    // Next to them, at the spacing of their lines, the line of the band that
    // changes from page to page (the name of the current section).
    let size = |(_, gs): &(i64, Vec<usize>)| gs.iter().map(|&g| page.glyphs[g].size).fold(1.0, f64::max);
    let close = |a: &(i64, Vec<usize>), b: &(i64, Vec<usize>)| ((a.0 - b.0).abs() as f64) <= size(a).max(size(b)) * 1.6;
    let mut top = lines.iter().take_while(|l| running(l)).count();
    while top > 0 && top < lines.len() && close(&lines[top - 1], &lines[top]) { top += 1 }
    let mut bottom = lines.iter().rev().take(lines.len() - top).take_while(|l| running(l)).count();
    while bottom > 0 && bottom < lines.len() - top && close(&lines[lines.len() - bottom], &lines[lines.len() - bottom - 1]) { bottom += 1 }
    lines[..top].iter().chain(&lines[lines.len() - bottom..]).flat_map(|(_, gs)| gs.iter().copied()).collect()
}

/// A page number alone: up to four digits or Roman numerals, perhaps between
/// dashes ("12", "- 3 -", "iv"). A single letter is not one: on a page set
/// sideways every letter is a line of its own.
fn page_number(text: &str) -> bool {
    let marks: Vec<char> = text.chars().filter(|c| !c.is_whitespace() && !matches!(c, '-' | '–' | '—' | '.' | '/')).collect();
    !marks.is_empty() && marks.len() <= 4
        && (marks.iter().all(char::is_ascii_digit) || marks.iter().all(|c| "ivxlcIVXLC".contains(*c)) && marks.len() > 1)
}

/// A line's words for comparing running headers: no numbers, no short words,
/// in any order (a footer mirrored on even and odd pages).
fn band_key(text: &str) -> String {
    let mut words: Vec<String> = text.split_whitespace().map(|w| w.chars().filter(|c| !c.is_ascii_digit()).collect::<String>()).filter(|w| w.chars().count() > 3).collect();
    words.sort();
    words.join(" ")
}

/// The band keys of every line of a page, by height.
fn band_keys(page: &Page) -> HashSet<(i64, String)> {
    let mut by_line: std::collections::BTreeMap<i64, Vec<&Glyph>> = Default::default();
    for glyph in &page.glyphs { by_line.entry(glyph.y.round() as i64).or_default().push(glyph) }
    by_line.into_iter().map(|(y, mut gs)| { gs.sort_by(|a, b| a.x.total_cmp(&b.x)); (y, band_key(&gs.iter().map(|g| g.text.as_str()).collect::<String>())) }).filter(|(_, k)| !k.is_empty()).collect()
}

/// The lines of a table of contents: a title, dot leaders ("....." or "…")
/// and a page number at the end. Their glyphs.
fn leader_lines(page: &Page) -> HashSet<usize> {
    let mut by_line: std::collections::BTreeMap<i64, Vec<usize>> = Default::default();
    for (g, glyph) in page.glyphs.iter().enumerate() { by_line.entry(glyph.y.round() as i64).or_default().push(g) }
    by_line.into_values().filter(|gs| {
        let mut sorted: Vec<&Glyph> = gs.iter().map(|&g| &page.glyphs[g]).collect();
        sorted.sort_by(|a, b| a.x.total_cmp(&b.x));
        let text: String = sorted.iter().map(|g| g.text.as_str()).collect::<String>().replace('…', "...");
        let text = text.trim_end();
        let number = text.trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, 'i' | 'v' | 'x'));
        number.len() < text.len() && number.trim_end().ends_with("....")
    }).flatten().collect()
}

/// The value of a Roman numeral written the usual way ("iv" → 4, "XII" →
/// 12); None for any other word, even one of its letters ("civil", "mix").
fn roman(text: &str) -> Option<u32> {
    const STEPS: [(u32, &str); 13] = [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")];
    let lower = text.to_lowercase();
    let mut rest = lower.as_str();
    let mut value = 0;
    for (n, mark) in STEPS {
        while let Some(r) = rest.strip_prefix(mark) { value += n; rest = r }
    }
    (rest.is_empty() && value > 0 && to_roman(value) == lower).then_some(value)
}

fn to_roman(mut n: u32) -> String {
    const STEPS: [(u32, &str); 13] = [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")];
    let mut out = String::new();
    for (v, mark) in STEPS { while n >= v { out.push_str(mark); n -= v } }
    out
}

/// The numbers written in a page's running header and footer ("Pág. 9",
/// "- 9 -", "Página 9 de 30", "iv"): each with its line's height, its value
/// and how it is printed. Which of them is the page's own number is decided
/// against the pages around it (see `printed_pages`).
fn folio_candidates(page: &Page, band: &HashSet<usize>) -> Vec<(i64, u32, String)> {
    let mut by_line: std::collections::BTreeMap<i64, Vec<&Glyph>> = Default::default();
    for &g in band { by_line.entry(page.glyphs[g].y.round() as i64).or_default().push(&page.glyphs[g]) }
    let mut out = Vec::new();
    for (y, mut gs) in by_line {
        gs.sort_by(|a, b| a.x.total_cmp(&b.x));
        let text: String = gs.iter().map(|g| g.text.as_str()).collect();
        for token in text.split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()) {
            let value = if token.chars().all(|c| c.is_ascii_digit()) { token.parse().ok().filter(|_| token.len() <= 4) } else { roman(token) };
            if let Some(v) = value { out.push((y, v, token.to_string())) }
        }
    }
    out
}

/// Each page's own printed number, from the numbers of its running header
/// and footer: the one that goes up by one from the page before or to the
/// page after at about the same height. A year or a "de 30" that every page
/// repeats never does, nor a section number, which stays. A page without one
/// (a cover) has none.
fn printed_pages(candidates: &[Vec<(i64, u32, String)>]) -> HashMap<u32, String> {
    let near = |a: i64, b: i64| (a - b).abs() <= 2;
    let follows = |page: Option<&Vec<(i64, u32, String)>>, y: i64, value: Option<u32>| {
        value.is_some_and(|v| page.is_some_and(|p| p.iter().any(|(y2, v2, _)| near(y, *y2) && *v2 == v)))
    };
    let mut out = HashMap::new();
    for (i, page) in candidates.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|p| candidates.get(p));
        let after = candidates.get(i + 1);
        let score = |(y, v, _): &&(i64, u32, String)| u8::from(follows(before, *y, v.checked_sub(1))) + u8::from(follows(after, *y, Some(v + 1)));
        if let Some(best) = page.iter().filter(|c| score(c) > 0).max_by_key(score) {
            out.insert(i as u32 + 1, best.2.clone());
        }
    }
    out
}

/// The page numbers the PDF itself declares (/PageLabels in its catalog:
/// "i, ii, iii, 1, 2…", "A-1, A-2…"), as a viewer shows them.
fn page_labels(pdf: &lopdf::Document, count: u32) -> HashMap<u32, String> {
    let deref = |o: &Object| pdf.dereference(o).ok().map(|(_, o)| o.clone());
    let mut ranges: Vec<(u32, Dictionary)> = Vec::new();
    let mut nodes: Vec<(Object, u8)> = pdf.catalog().ok().and_then(|c| c.get(b"PageLabels").ok()).map(|o| vec![(o.clone(), 0)]).unwrap_or_default();
    while let Some((node, depth)) = nodes.pop() {
        let Some(Object::Dictionary(node)) = deref(&node) else { continue };
        if let Some(Object::Array(nums)) = node.get(b"Nums").ok().and_then(deref) {
            for pair in nums.chunks_exact(2) {
                if let (Ok(start), Some(Object::Dictionary(style))) = (pair[0].as_i64(), deref(&pair[1])) {
                    if start >= 0 { ranges.push((start as u32, style)) }
                }
            }
        }
        if depth < 16 {
            if let Some(Object::Array(kids)) = node.get(b"Kids").ok().and_then(deref) { nodes.extend(kids.into_iter().map(|k| (k, depth + 1))) }
        }
    }
    ranges.sort_by_key(|r| r.0);
    let mut out = HashMap::new();
    for (k, (start, style)) in ranges.iter().enumerate() {
        let end = ranges.get(k + 1).map_or(count, |r| r.0.min(count));
        let prefix = style.get(b"P").ok().and_then(|o| o.as_str().ok()).map(crate::about::pdf_string).unwrap_or_default();
        let first = style.get(b"St").ok().and_then(|o| o.as_i64().ok()).filter(|n| *n > 0).unwrap_or(1) as u32;
        let kind = style.get(b"S").ok().and_then(|o| o.as_name().ok()).map(|n| n.to_vec());
        for index in *start..end {
            let n = first + index - start;
            let letters = |upper: bool| {
                let c = char::from(b'a' + ((n - 1) % 26) as u8);
                let c = if upper { c.to_ascii_uppercase() } else { c };
                c.to_string().repeat(((n - 1) / 26 + 1) as usize)
            };
            let number = match kind.as_deref() {
                Some(b"D") => n.to_string(),
                Some(b"R") => to_roman(n).to_uppercase(),
                Some(b"r") => to_roman(n),
                Some(b"A") => letters(true),
                Some(b"a") => letters(false),
                _ => String::new(),
            };
            let label = format!("{prefix}{number}");
            if !label.trim().is_empty() { out.insert(index + 1, label); }
        }
    }
    out
}

/// "la página 3", "las páginas 1–4, 7".
fn pages_list(pages: &[u32]) -> String {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for &p in pages {
        match runs.last_mut() { Some(r) if r.1 + 1 == p => r.1 = p, _ => runs.push((p, p)) }
    }
    let text = runs.iter().map(|&(a, b)| if a == b { a.to_string() } else { format!("{a}–{b}") }).collect::<Vec<_>>().join(", ");
    if pages.len() == 1 { format!("la página {text}") } else { format!("las páginas {text}") }
}

pub fn extract(bytes: &[u8]) -> Result<Document, String> {
    let mut pdf = pdf_extract::Document::load_mem(bytes).map_err(|e| format!("PDF ilegible: {e}"))?;
    if pdf.is_encrypted() && pdf.decrypt("").is_err() { return Err("PDF protegido con contraseña".into()) }
    let page_ids = pdf.get_pages();
    let numbers: Vec<u32> = page_ids.keys().copied().collect();
    let count = numbers.len() as u32;
    if count == 0 { return Err("PDF sin páginas".into()) }
    let mut doc = Document { kind: "pdf", pages: Some(count), about: crate::about::pdf(&pdf), ..Default::default() };
    let mut collector = Collector::default();
    let mut unreadable = Vec::new();
    for n in &numbers {
        let before = collector.pages.len();
        if pdf_extract::output_doc_page(&pdf, &mut collector, *n).is_err() {
            collector.pages.truncate(before);
            collector.pages.push(Page::default());
            unreadable.push(*n);
        }
        if let Some(page) = collector.pages.last_mut() { draw(&pdf, page_ids[n], page); upright(page) }
    }
    if !unreadable.is_empty() { doc.notes.push(format!("Páginas que no se pudieron leer: {unreadable:?}.")) }

    // Tables, page by page; one that goes on from the bottom of a page to the
    // top of the next with the same columns is the same table. Between the two
    // there may only be the running header and footer: lines at the same
    // height with the same text on both pages (the page number aside).
    // The body text's color: the one most letters are painted with.
    let mut painted: HashMap<String, usize> = HashMap::new();
    for g in collector.pages.iter().flat_map(|p| &p.glyphs).filter(|g| !g.text.trim().is_empty()) {
        if let Some(c) = &g.color { *painted.entry(c.clone()).or_default() += 1 }
    }
    let body = painted.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c).unwrap_or_else(|| color::hex(0.0, 0.0, 0.0));
    let lines: Vec<HashSet<(i64, String)>> = collector.pages.iter().map(|p| line_keys(p, |_| true)).collect();
    let bands: Vec<HashSet<(i64, String)>> = collector.pages.iter().map(band_keys).collect();
    let mut pages_text = Vec::new();
    // The table that may go on at the top of the next page: its id, columns
    // and the page it ended on.
    let mut last: Option<(usize, Vec<f64>, Vec<f64>, bool, u32)> = None;
    let (mut by_alignment, mut doubtful) = (Vec::new(), Vec::new());
    let mut folios = Vec::new();
    for (i, page) in collector.pages.iter().enumerate() {
        let number = i as u32 + 1;
        let mut found = grids(page);
        let neighbors: Vec<&HashSet<(i64, String)>> = [i.checked_sub(1), Some(i + 1)].into_iter().flatten().filter_map(|p| bands.get(p)).collect();
        let mut taken: HashSet<usize> = found.iter().flat_map(|g| g.members.iter().copied()).collect();
        let band = running_band(page, &neighbors);
        folios.push(folio_candidates(page, &band));
        taken.extend(band.iter().copied());
        taken.extend(leader_lines(page));
        let (aligned, unsure) = streams(page, &taken);
        if !aligned.is_empty() { by_alignment.push(number) } else if unsure { doubtful.push(number) }
        found.extend(aligned);
        found.sort_by(|a, b| b.top.total_cmp(&a.top));
        // The lines outside tables and outside the running header and footer
        // the page before has too (a caption repeated from this page on is
        // still a title here).
        let before = i.checked_sub(1).and_then(|p| bands.get(p));
        let key_at: HashMap<i64, &String> = bands[i].iter().map(|(y, k)| (*y, k)).collect();
        let carried = |g: usize| { let y = page.glyphs[g].y.round() as i64; key_at.get(&y).is_some_and(|k| before.is_some_and(|b| !b.contains(&(y, (*k).clone())))) };
        // Those running lines also leave the page's text: the same copyright,
        // title and page number on every page are noise in every section and
        // every search. A line of the band that changed since the page before
        // (the name of a new section) stays.
        let hidden: HashSet<usize> = band.iter().copied().filter(|&g| !carried(g)).collect();
        let taken: HashSet<usize> = found.iter().flat_map(|g| g.members.iter().copied()).chain(hidden.iter().copied()).collect();
        let free = text_lines(page, &taken);
        let mut table_of = HashMap::new();
        for (k, grid) in found.iter().enumerate() {
            let inside: HashSet<usize> = grid.members.iter().copied().collect();
            let running = |keys: HashSet<(i64, String)>, other: Option<&HashSet<(i64, String)>>| keys.is_empty() || other.is_some_and(|o| keys.is_subset(o));
            // Between a table and the edge of the page only the running header
            // or footer, or lines repeated on the next (or previous) page.
            let below_all = running(line_keys(page, |(g, gl)| !inside.contains(&g) && !band.contains(&g) && gl.y < grid.bottom), lines.get(i + 1));
            let above_all = running(line_keys(page, |(g, gl)| !inside.contains(&g) && !band.contains(&g) && gl.y > grid.top), i.checked_sub(1).and_then(|p| lines.get(p)));
            // Same columns: each column line where the other's is, within how
            // far either may move (a lane's width for a table without borders).
            let same_columns = |xs: &Vec<f64>, slack: &Vec<f64>, stream: bool| stream == grid.stream && xs.len() == grid.xs.len()
                && xs.iter().zip(&grid.xs).zip(slack.iter().zip(&grid.slack)).all(|((a, b), (s, t))| (a - b).abs() <= s + t + SNAP * 2.0);
            // Above a table that goes on, the page may repeat its title with
            // more words ("… (cont.)"): a line or two holding every word of it.
            let caption: Vec<&Line> = free.iter().filter(|l| l.y > grid.top - l.size * 0.5).collect();
            let repeats_title = |t: usize| caption.len() <= 2 && {
                let words = |text: &str| text.split_whitespace().map(str::to_lowercase).filter(|w| w.chars().count() > 3).collect::<HashSet<String>>();
                let title = words(&doc.tables[t].title);
                let above = words(&caption.iter().map(|l| line_text(page, l, f64::MIN, f64::MAX)).collect::<Vec<_>>().join(" "));
                !title.is_empty() && title.is_subset(&above)
            };
            let id = match &last {
                Some((t, xs, slack, stream, p)) if k == 0 && *p + 1 == number && (above_all || repeats_title(*t)) && same_columns(xs, slack, *stream) => {
                    let mut rows = grid.rows.clone();
                    // A header row repeated on every page is written once.
                    let skip = usize::from(rows.first() == doc.tables[*t].rows.first());
                    if skip == 1 { rows.remove(0); }
                    let base = doc.tables[*t].rows.len();
                    let colors = grid_colors(page, grid, &body, skip, base);
                    doc.tables[*t].colors.extend(colors);
                    doc.tables[*t].rows.extend(rows);
                    if !doc.tables[*t].pages.contains(&number) { doc.tables[*t].pages.push(number) }
                    *t
                }
                _ => {
                    // Its title: the lines just above it (a caption, a heading),
                    // as many as follow each other at the spacing of a paragraph.
                    let mut title_lines: Vec<&Line> = Vec::new();
                    if let Some(k) = free.iter().rposition(|l| l.y > grid.top - l.size * 0.5 && l.y - grid.top <= l.size * 2.5) {
                        title_lines.push(&free[k]);
                        for j in (0..k).rev() {
                            let below = title_lines.last().unwrap();
                            if free[j].y - below.y > below.size * 1.6 { break }
                            title_lines.push(&free[j]);
                        }
                    }
                    let title = title_lines.iter().rev().map(|l| line_text(page, l, f64::MIN, f64::MAX)).collect::<Vec<_>>().join(" ");
                    doc.tables.push(Table { title, page: Some(number), pages: vec![number], rows: grid.rows.clone(), colors: grid_colors(page, grid, &body, 0, 0), ..Default::default() });
                    doc.tables.len() - 1
                }
            };
            for &g in &grid.members { table_of.insert(g, id); }
            last = if k == found.len() - 1 && below_all { Some((id, grid.xs.clone(), grid.slack.clone(), grid.stream, number)) } else { None };
        }
        if found.is_empty() { last = None }
        let skip: HashSet<usize> = table_of.keys().copied().chain(hidden.iter().copied()).collect();
        let (text, pieces) = page_text(page, &table_of, &hidden, &text_marks(page, &skip, &body));
        for (c, t) in pieces { doc.colors.add("texto", &c, &t) }
        pages_text.push(text);
    }
    for table in &doc.tables {
        for c in &table.colors {
            if let Some(f) = &c.fill { doc.colors.add("fondo", f, table.rows.get(c.row).and_then(|r| r.get(c.column)).map(String::as_str).unwrap_or("")) }
            for m in &c.text { if let Some(k) = &m.color { doc.colors.add("texto", k, &m.text) } }
        }
    }
    if !by_alignment.is_empty() {
        doc.notes.push(format!("Tablas sin bordes leídas por la alineación de sus columnas en {}: si una celda decide algo, verifícala con la imagen de su página.", pages_list(&by_alignment)));
    }
    if !doubtful.is_empty() {
        let list = pages_list(&doubtful);
        doc.notes.push(format!("{}{} parece{} tener tablas sin bordes que quedaron como texto en su sección: verifícalas con la imagen de la página (paginas/pNNN.png).", list[..1].to_uppercase(), &list[1..], if doubtful.len() == 1 { "" } else { "n" }));
    }
    // The document's own numbering checks the tables: a caption ("Table 16:")
    // left in the text has no CSV (its rows stayed as text), and one inside
    // another table's cells was taken by that table. Both are named, with
    // their page, and their pages get an image to look at.
    let key = |text: &str| {
        let mut words = text.split_whitespace();
        format!("{} {}", words.next().unwrap_or("").to_lowercase(), words.next().unwrap_or("").trim_end_matches([':', '.']))
    };
    let titled: HashSet<String> = doc.tables.iter().filter(|t| caption(&t.title)).map(|t| key(&t.title)).collect();
    let mut orphans: Vec<(u32, String)> = Vec::new();
    for (i, text) in pages_text.iter().enumerate() {
        for line in text.lines().map(color::strip_tags) {
            let line = line.trim();
            // A list of tables names them all, with leaders to their pages.
            if caption(line) && !figure(line) && !line.contains("....") && !titled.contains(&key(line)) { orphans.push((i as u32 + 1, line.chars().take(70).collect())) }
        }
    }
    let mut taken_by: Vec<u32> = Vec::new();
    for table in &mut doc.tables {
        let own = key(&table.title);
        let inside: Vec<String> = table.rows.iter().flatten().flat_map(|c| c.lines()).map(str::trim).filter(|l| caption(l) && key(l) != own).map(|l| l.chars().take(70).collect()).collect();
        if !inside.is_empty() {
            table.check = format!("tiene dentro el título de otra tabla («{}»): puede llevar filas que no son suyas", inside.join("», «"));
            taken_by.extend(table.pages.iter().copied());
        }
    }
    if !orphans.is_empty() {
        let shown: Vec<String> = orphans.iter().take(10).map(|(p, t)| format!("«{t}» (página {p})")).collect();
        let more = if orphans.len() > 10 { format!(" y {} más", orphans.len() - 10) } else { String::new() };
        let one = orphans.len() == 1;
        doc.notes.push(format!("{} tabla{} que el documento numera {} sin CSV propio, como texto en su sección: {}{more}. {} con la imagen de su página.", orphans.len(), if one { "" } else { "s" }, if one { "quedó" } else { "quedaron" }, shown.join(", "), if one { "Verifícala" } else { "Verifícalas" }));
    }
    if !taken_by.is_empty() {
        let n = doc.tables.iter().filter(|t| !t.check.is_empty()).count();
        doc.notes.push(format!("{n} tabla{} dentro el título de otra (marcada{} con check en tablas/index.json): {} con la imagen de su página.", if n == 1 { " lleva" } else { "s llevan" }, if n == 1 { "" } else { "s" }, if n == 1 { "verifícala" } else { "verifícalas" }));
    }
    let mut check: Vec<u32> = doubtful.into_iter().chain(orphans.iter().map(|o| o.0)).chain(taken_by).collect();
    check.sort();
    check.dedup();
    doc.check_pages = check;

    // A page that is mostly a picture (a scan, a photo of a page) is read by
    // OCR; its text replaces the page's own only when OCR finds more letters.
    let letters = |t: &str| t.chars().filter(|c| c.is_alphanumeric()).count();
    let pictures: Vec<u32> = collector.pages.iter().enumerate().filter(|(_, p)| p.area > 0.0 && p.image_area >= p.area / 2.0).map(|(i, _)| i as u32 + 1).collect();
    let mut by_ocr = HashSet::new();
    if !pictures.is_empty() {
        match crate::render::ocr(bytes, &pictures) {
            Ok(ocr) => {
                for (n, text) in ocr.pages {
                    let i = (n - 1) as usize;
                    if letters(&text) > letters(&pages_text[i]) { pages_text[i] = text; by_ocr.insert(n); }
                }
                if !by_ocr.is_empty() {
                    let mut used: Vec<u32> = by_ocr.iter().copied().collect();
                    used.sort();
                    doc.notes.push(format!("Texto de {} leído con {} sobre la imagen de la página: puede tener errores de reconocimiento.", pages_list(&used), ocr.engine));
                    doc.ocr = Some(ocr.engine);
                }
            }
            Err(e) => doc.notes.push(format!("No se pudo leer el texto de {}, que {} ({e}): mira paginas/.", pages_list(&pictures), if pictures.len() == 1 { "es una imagen" } else { "son imágenes" })),
        }
    }
    // Bookmarks below level 2 are the document's own subheadings (a field, a
    // subfield): their line on their page becomes a Markdown heading, so a long
    // section is read, cut in parts and searched by them. Of several lines with
    // the title, the last (the running header above repeats it).
    if let Ok(toc) = pdf.get_toc() {
        let plain = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        for item in toc.toc.iter().filter(|t| t.level > 2 && t.page >= 1 && t.page as u32 <= count) {
            let title = plain(&item.title);
            if title.chars().count() < 3 { continue }
            let text = &mut pages_text[item.page - 1];
            let lines: Vec<&str> = text.lines().collect();
            let Some(at) = lines.iter().rposition(|l| !l.starts_with('#') && plain(&color::strip_tags(l)) == title) else { continue };
            let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            out[at] = format!("{} {}", "#".repeat(item.level.min(6)), out[at].trim());
            *text = out.join("\n");
        }
    }
    // The page numbers printed on the pages, which a contract or a set of
    // rules is quoted by ("Pág. 9"), when they are not the PDF's own (a cover,
    // an index, annexes numbered apart): the number in the running header or
    // footer, else the numbering the PDF declares.
    let mut printed = page_labels(&pdf, count);
    printed.extend(printed_pages(&folios));
    printed.retain(|p, label| *label != p.to_string());
    if let Some((p, label)) = printed.iter().min_by_key(|(p, _)| **p) {
        doc.notes.push(format!("La numeración impresa no es la del PDF (la página {p} del PDF dice «{label}»): cada marca de página, sección y tabla lleva también la impresa; cítala así y pide pages por la del PDF."));
    }
    doc.printed = printed.iter().map(|(p, l)| (*p, l.clone())).collect();
    let text_of = |from: u32, to: u32| -> String {
        (from..=to).map(|p| {
            let how = if by_ocr.contains(&p) { " · texto por OCR" } else { "" };
            let label = printed.get(&p).map(|l| format!(" · impresa {l}")).unwrap_or_default();
            format!("<!-- página {p}{label}{how} -->\n{}\n", pages_text[(p - 1) as usize].trim())
        }).collect::<Vec<_>>().join("\n")
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
            let first = pages_text[(p - 1) as usize].lines().map(|l| color::strip_tags(l).trim().to_string()).find(|l| !l.is_empty() && !l.starts_with("[Tabla")).unwrap_or_default().chars().take(70).collect::<String>();
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
            if table.title.is_empty() { table.title = section.title.clone() }
            section.tables.push(t);
        }
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_number_is_digits_or_roman_numerals_never_a_lone_letter() {
        for yes in ["12", "- 3 -", "1088", "iv", "XII"] { assert!(page_number(yes), "{yes:?}") }
        // On a page set sideways each letter is a line of its own in the margin.
        for no in ["R", "e", "V S", "ab", "12345", ""] { assert!(!page_number(no), "{no:?}") }
    }

    #[test]
    fn a_roman_numeral_is_only_one_written_the_usual_way() {
        for (text, n) in [("iv", 4), ("XII", 12), ("xl", 40), ("i", 1)] { assert_eq!(roman(text), Some(n), "{text}") }
        for no in ["iiii", "vx", "civil", "Pág", ""] { assert_eq!(roman(no), None, "{no}") }
    }

    #[test]
    fn captions_name_a_table_or_figure_and_its_number() {
        for yes in ["Table 12: Fields", "Table 8–34: Fee schedule", "Tabla 3. Ciclos", "Figure 7 – Flow"] { assert!(caption(yes), "{yes:?}") }
        for no in ["Table of contents", "Tables 1 to 4 list", "Refer to Table 12 for", "Table"] { assert!(!caption(no), "{no:?}") }
        assert!(figure("Figure 7 – Flow") && !figure("Table 12: Fields"));
    }
}

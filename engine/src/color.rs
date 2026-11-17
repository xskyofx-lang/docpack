//! Colors a document uses to mark things (the dogs in red, a gray row that
//! is not delivered yet): which ones are marks, their name,
//! and the legend of a document. A color is kept as its hex value ("FF0000")
//! and named by its hue, so "the pink ones" can be found whatever pink a
//! writer chose.

/// "FF0000" from red, green and blue in 0-1.
pub fn hex(r: f64, g: f64, b: f64) -> String {
    let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{:02X}{:02X}{:02X}", c(r), c(g), c(b))
}

/// Red, green and blue in 0-1 of "FF0000" (or "ff0000"); None for anything else.
pub fn rgb(hex: &str) -> Option<(f64, f64, f64)> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 { return None }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|x| x as f64 / 255.0);
    Some((v(0)?, v(2)?, v(4)?))
}

/// The hex value of a color as a document writes it: "FF0000", "auto" (none).
pub fn normal(value: &str) -> Option<String> {
    rgb(value).map(|(r, g, b)| hex(r, g, b))
}

/// Word's highlight colors (`w:highlight`), which are names, not values.
pub fn highlight(name: &str) -> Option<String> {
    let v = match name {
        "yellow" => "FFFF00", "green" => "00FF00", "cyan" => "00FFFF", "magenta" => "FF00FF",
        "blue" => "0000FF", "red" => "FF0000", "darkBlue" => "000080", "darkCyan" => "008080",
        "darkGreen" => "008000", "darkMagenta" => "800080", "darkRed" => "800000", "darkYellow" => "808000",
        "darkGray" => "808080", "lightGray" => "C0C0C0", "black" => "000000",
        _ => return None, // "none", "white"
    };
    Some(v.into())
}

/// Hue (0-360), saturation and lightness (0-1).
fn hsl((r, g, b): (f64, f64, f64)) -> (f64, f64, f64) {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-9 { return (0.0, 0.0, l) }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r { ((g - b) / d).rem_euclid(6.0) } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 } * 60.0;
    (h, s, l)
}

/// Office's tint (-1 to 1) on a color: a negative tint darkens it toward
/// black, a positive one lightens it toward white, on its HSL lightness
/// (ECMA-376 §18.8.19). Theme colors are written as a base and a tint.
pub fn tint(hex_value: &str, tint: f64) -> Option<String> {
    let (h, s, l) = hsl(rgb(hex_value)?);
    let l = if tint < 0.0 { l * (1.0 + tint) } else { l * (1.0 - tint) + tint };
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 { 0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x), 3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x) };
    let m = l - c / 2.0;
    Some(hex(r + m, g + m, b + m))
}

/// Lightness as the eye sees it, 0 (black) to 1 (white).
fn luma((r, g, b): (f64, f64, f64)) -> f64 { 0.2126 * r + 0.7152 * g + 0.0722 * b }

/// The color's name in Spanish, by hue and lightness: rojo, rosa, naranja,
/// café, amarillo, verde, turquesa, azul, morado, gris, negro, blanco.
pub fn name(hex: &str) -> &'static str {
    let Some(c) = rgb(hex) else { return "color" };
    let (h, s, l) = hsl(c);
    if s < 0.15 || l < 0.08 || l > 0.96 {
        return if l < 0.15 { "negro" } else if l > 0.96 { "blanco" } else { "gris" };
    }
    match h {
        h if !(15.0..335.0).contains(&h) => if l > 0.7 { "rosa" } else { "rojo" },
        // Very light, toward yellow, it reads as yellow (Office's gold at
        // 80 % lighter, FFF2CC, is the "light yellow" of a spreadsheet).
        h if h < 50.0 => if l < 0.35 { "café" } else if h >= 40.0 && l > 0.8 { "amarillo" } else { "naranja" },
        h if h < 70.0 => "amarillo",
        h if h < 165.0 => "verde",
        h if h < 255.0 && l > 0.75 => "celeste",
        h if h < 195.0 => "turquesa",
        h if h < 255.0 => "azul",
        h if h < 290.0 => "morado",
        // Magenta: dark it reads as purple, bright or light as pink.
        _ => if l < 0.35 { "morado" } else { "rosa" },
    }
}

/// The color name a person asked for, as `name` writes it, in Spanish or in
/// English: "pink" and "rosa" are the same search. Lowercase, without
/// accents ("cafe"); None for a name DocPack does not use.
pub fn named(asked: &str) -> Option<&'static str> {
    let asked: String = asked.trim().to_lowercase().replace(['_', '-'], " ").split_whitespace().collect::<Vec<_>>().join(" ");
    Some(match asked.as_str() {
        "rojo" | "red" => "rojo",
        "rosa" | "pink" | "magenta" | "fuchsia" => "rosa",
        "naranja" | "orange" => "naranja",
        "café" | "cafe" | "marrón" | "marron" | "brown" => "café",
        "amarillo" | "yellow" | "gold" => "amarillo",
        "verde" | "green" => "verde",
        "celeste" | "light blue" | "lightblue" | "sky blue" | "skyblue" => "celeste",
        "turquesa" | "turquoise" | "teal" | "cyan" | "aqua" => "turquesa",
        "azul" | "blue" | "navy" => "azul",
        "morado" | "purple" | "violet" | "lila" => "morado",
        "gris" | "gray" | "grey" => "gris",
        "negro" | "black" => "negro",
        "blanco" | "white" => "blanco",
        _ => return None,
    })
}

/// "FF0000 (rojo)".
pub fn label(hex: &str) -> String { format!("{hex} ({})", name(hex)) }

/// Text marked in a section's Markdown with the HTML Markdown keeps:
/// `<span style="color:#FF0000">` for colored text, `<mark
/// style="background:#FFFF00">` for a highlight or a fill.
pub fn wrap(text: &str, color: Option<&str>, fill: Option<&str>) -> String {
    let mut t = text.to_string();
    if let Some(c) = color { t = format!("<span style=\"color:#{c}\">{t}</span>") }
    if let Some(f) = fill { t = format!("<mark style=\"background:#{f}\">{t}</mark>") }
    t
}

/// Text in `hex` is marked when it is a color (not a gray) or clearly lighter
/// or darker than the document's body text: a heading in a dark navy or a
/// dark gray is the document's style, red is a mark. White text is the
/// document's own contrast on a dark fill, not a mark.
pub fn marks_text(hex: &str, body: &str) -> bool {
    let (Some(c), Some(b)) = (rgb(hex), rgb(body)) else { return false };
    if luma(c) > 0.9 { return false }
    let (_, s, l) = hsl(c);
    let chroma = s * (1.0 - (2.0 * l - 1.0).abs());
    chroma >= 0.25 || (luma(c) - luma(b)).abs() >= 0.35
}

/// A background marks its text when it is not white (a cell's gray, a
/// highlight in yellow): every fill but white and near white.
pub fn marks_fill(hex: &str) -> bool {
    rgb(hex).is_some_and(|c| luma(c) < 0.94 || hsl(c).1 > 0.25 && luma(c) < 0.985)
}

/// A line of a section without the color tags DocPack adds.
pub fn strip_tags(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find('<') {
        let tag = &rest[at..];
        let ours = ["<span style=", "</span>", "<mark style=", "</mark>"].iter().any(|t| tag.starts_with(t));
        match (ours, tag.find('>')) {
            (true, Some(end)) => { out.push_str(&rest[..at]); rest = &rest[at + end + 1..] }
            _ => { out.push_str(&rest[..=at]); rest = &rest[at + 1..] }
        }
    }
    out.push_str(rest);
    out
}

/// How a color was used, how many times and with what text.
#[derive(Debug, Clone, Default)]
pub struct Use { pub count: usize, pub samples: Vec<String> }

/// The colors a document marks with: (kind, hex) → use. Kind is "texto",
/// "resaltado" or "fondo".
#[derive(Debug, Clone, Default)]
pub struct Legend(pub std::collections::BTreeMap<(String, String), Use>);

impl Legend {
    pub fn add(&mut self, kind: &str, hex: &str, text: &str) {
        let entry = self.0.entry((kind.to_string(), hex.to_string())).or_default();
        entry.count += 1;
        let sample: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect();
        if !sample.is_empty() && entry.samples.len() < 5 && !entry.samples.contains(&sample) { entry.samples.push(sample) }
    }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
    /// Most used first.
    pub fn entries(&self) -> Vec<(&str, &str, &Use)> {
        let mut out: Vec<_> = self.0.iter().map(|((k, h), u)| (k.as_str(), h.as_str(), u)).collect();
        out.sort_by(|a, b| b.2.count.cmp(&a.2.count).then(a.0.cmp(b.0)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_named_by_their_hue() {
        for (hex, want) in [("FF0000", "rojo"), ("C00000", "rojo"), ("FF99CC", "rosa"), ("FF00FF", "rosa"), ("7030A0", "morado"), ("800080", "morado"),
                            ("0070C0", "azul"), ("00B050", "verde"), ("FFFF00", "amarillo"), ("FFC000", "naranja"), ("BFBFBF", "gris"), ("EAEAEA", "gris"), ("000000", "negro"), ("FFFFFF", "blanco"), ("45818E", "turquesa"), ("CFE2F3", "celeste"), ("A6E4F8", "celeste"),
                            // Office's standard colors.
                            ("92D050", "verde"), ("00B0F0", "azul"), ("002060", "azul")] {
            assert_eq!(name(hex), want, "{hex}");
        }
    }

    #[test]
    fn a_mark_is_a_color_not_the_body_style() {
        assert!(marks_text("FF0000", "000000") && marks_text("7030A0", "000000") && marks_text("808080", "000000"));
        for style in ["000000", "222222", "434343", "0F243E", "FFFFFF"] { assert!(!marks_text(style, "000000"), "{style}") }
        assert!(marks_fill("BFBFBF") && marks_fill("FFFF00") && marks_fill("CFE2F3"));
        assert!(!marks_fill("FFFFFF") && !marks_fill("F6F8F9"));
    }
}

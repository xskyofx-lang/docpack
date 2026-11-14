//! Colors a document marks things with: the dogs in red (vaccinated), a gray
//! header, a pink "check this". Kept beside the plain text
//! (the CSV does not change) and named, so "the pink ones" can be asked for.
use docpack::{extract, CellColor};
use std::io::Write;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn docx(styles: &str, body: &str) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut out);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("word/document.xml", o).unwrap();
        z.write_all(format!(r#"<w:document {W}><w:body>{body}</w:body></w:document>"#).as_bytes()).unwrap();
        z.start_file("word/styles.xml", o).unwrap();
        z.write_all(format!(r#"<w:styles {W}>{styles}</w:styles>"#).as_bytes()).unwrap();
        z.finish().unwrap();
    }
    out.into_inner()
}

fn run(text: &str, props: &str) -> String { format!(r#"<w:r><w:rPr>{props}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r>"#) }
fn cell(fill: Option<&str>, runs: &str) -> String {
    let shd = fill.map(|f| format!(r#"<w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="{f}"/></w:tcPr>"#)).unwrap_or_default();
    format!("<w:tc>{shd}<w:p>{runs}</w:p></w:tc>")
}
fn text(doc: &docpack::Document) -> String { doc.sections.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n") }
fn at(colors: &[CellColor], row: usize, column: usize) -> &CellColor {
    colors.iter().find(|c| c.row == row && c.column == column).unwrap_or_else(|| panic!("sin color en [{row},{column}]: {colors:?}"))
}

#[test]
fn word_keeps_the_color_of_each_piece_and_of_each_cell() {
    let styles = r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
        <w:style w:type="character" w:styleId="Vencida"><w:name w:val="Vencida"/><w:rPr><w:color w:val="C00000"/></w:rPr></w:style>
        <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:color w:val="0F243E"/></w:rPr></w:style>"#;
    let red = r#"<w:color w:val="FF0000"/>"#;
    let body = format!(
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>{}</w:p>
        <w:p>{}{}{}</w:p>
        <w:tbl>
          <w:tr>{}{}</w:tr>
          <w:tr>{}{}</w:tr>
          <w:tr>{}{}</w:tr>
        </w:tbl>"#,
        run("Alcance", ""),
        run("Los perros en ", ""), run("rojo", red), run(" ya están vacunados; los ", ""),
        cell(Some("BFBFBF"), &run("Mascota", "")), cell(Some("BFBFBF"), &run("Vacunas", "")),
        cell(None, &run("Perro", "")), cell(None, &format!("{}{}{}", run("rabia, ", red), run("tos, ", ""), run("moquillo", r#"<w:rStyle w:val="Vencida"/>"#))),
        cell(Some("auto"), &run("Gato", "")), cell(None, &run("Revisar", r#"<w:highlight w:val="magenta"/>"#)),
    );
    let doc = extract(&docx(styles, &body), "docx").unwrap();
    let all = text(&doc);
    assert!(all.contains(r#"Los perros en <span style="color:#FF0000">rojo</span> ya están vacunados"#), "{all}");
    assert!(!all.contains("0F243E"), "a heading in the document's navy is its style, not a mark: {all}");

    let t = &doc.tables[0];
    assert_eq!(t.rows[1], vec!["Perro", "rabia, tos, moquillo"], "the CSV keeps the plain text");
    assert_eq!(at(&t.colors, 0, 0).fill.as_deref(), Some("BFBFBF"));
    let vaccines = &at(&t.colors, 1, 1).text;
    assert_eq!(vaccines.iter().map(|m| (m.text.as_str(), m.color.as_deref())).collect::<Vec<_>>(), vec![("rabia,", Some("FF0000")), ("moquillo", Some("C00000"))],
        "red pieces, from the run and from its character style; tos stays black");
    assert_eq!(at(&t.colors, 2, 1).text[0].fill.as_deref(), Some("FF00FF"), "a highlight");
    assert!(!t.colors.iter().any(|c| c.row == 2 && c.column == 0), "auto is no fill");

    let legend: Vec<(&str, &str, usize)> = doc.colors.entries().iter().map(|(k, h, u)| (*k, *h, u.count)).collect();
    assert!(legend.contains(&("fondo", "BFBFBF", 2)) && legend.contains(&("texto", "FF0000", 2)) && legend.contains(&("resaltado", "FF00FF", 1)), "{legend:?}");
    assert_eq!(docpack::color::name("FF00FF"), "rosa");
}

#[test]
fn pdf_keeps_the_color_of_each_letter_and_the_fill_behind_a_cell() {
    let doc = extract(include_bytes!("fixtures/colores.pdf"), "pdf").unwrap();
    let all = text(&doc);
    assert!(all.contains(r#"Los perros en <span style="color:#FF0000">rojo</span> ya están vacunados"#), "{all}");
    assert!(all.contains("Tabla 1: Vacunas por mascota") && !all.contains("E66E1E"), "a heading all in orange is a style: {all}");

    let t = &doc.tables[0];
    assert_eq!(t.rows, vec![vec!["Mascota", "Vacunas"], vec!["Perro", "rabia, moquillo, tos, parvovirus"], vec!["Gato", "Pendiente"]]);
    assert_eq!(at(&t.colors, 0, 1).fill.as_deref(), Some("BFBFBF"), "gray header");
    let vaccines = &at(&t.colors, 1, 1).text;
    assert_eq!(vaccines.len(), 1, "{vaccines:?}");
    assert!(vaccines[0].text.starts_with("rabia, moquillo") && vaccines[0].color.as_deref() == Some("FF0000"), "only rabia and moquillo are red: {vaccines:?}");
    let pending = at(&t.colors, 2, 1);
    assert_eq!((pending.fill.as_deref(), pending.text[0].color.as_deref()), (Some("FFFF00"), Some("FF69B4")));
    assert_eq!(docpack::color::name("FF69B4"), "rosa");
    assert!(doc.colors.entries().iter().any(|(k, h, _)| *k == "texto" && *h == "FF0000"));
}

//! Word cases taken from a real technical design (numbered as in CHANGELOG.md, 0.1.3), rebuilt as small
//! synthetic documents.
use docpack::{build, extract, tokens_estimate, Options};
use std::io::Write;

const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:w16se="http://schemas.microsoft.com/office/word/2015/wordml/symex" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#;

fn docx(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut out);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("word/document.xml", o).unwrap();
        z.write_all(format!(r#"<w:document {W}><w:body>{body}</w:body></w:document>"#).as_bytes()).unwrap();
        z.start_file("word/styles.xml", o).unwrap();
        z.write_all(format!(r#"<w:styles {W}><w:style w:styleId="Heading2"><w:name w:val="heading 2"/></w:style></w:styles>"#).as_bytes()).unwrap();
        for (name, bytes) in extra { z.start_file(*name, o).unwrap(); z.write_all(bytes).unwrap(); }
        z.finish().unwrap();
    }
    out.into_inner()
}
fn p(text: &str) -> String { format!("<w:p><w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>") }
fn item(level: u8, text: &str) -> String { format!(r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#) }
fn all_text(doc: &docpack::Document) -> String { doc.sections.iter().map(|s| s.text.clone()).collect::<Vec<_>>().join("\n") }

// Falla 1: a symbol inserted as such keeps its character (it lives in mc:Choice).
#[test]
fn symbols_inserted_as_symbols_are_kept() {
    let arrow = r#"<w:p><w:r><w:t xml:space="preserve">id_proc </w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="w16se"><w16se:symEx w16se:font="Segoe UI Emoji" w16se:char="2192"/></mc:Choice><mc:Fallback><w:t>→</w:t></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t xml:space="preserve"> Identificador</w:t></w:r></w:p>"#;
    let doc = extract(&docx(arrow, &[]), "docx").unwrap();
    assert!(all_text(&doc).contains("id_proc → Identificador"), "{}", all_text(&doc));
    assert_eq!(all_text(&doc).matches('→').count(), 1, "read once, not from the fallback too");
}

// Falla 2: a line break inside a cell stays a line break.
#[test]
fn line_breaks_inside_a_cell_are_kept() {
    let table = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Campo</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Si es 01</w:t><w:br/><w:t>No sea 0000</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let doc = extract(&docx(table, &[]), "docx").unwrap();
    assert_eq!(doc.tables[0].rows[0][1], "Si es 01\nNo sea 0000");
}

// Fallas 3 y 6: list levels are kept and a paragraph after a list is apart.
#[test]
fn list_levels_are_kept_and_a_paragraph_after_a_list_stands_apart() {
    let body = [item(0, "ddmmaa"), item(1, "dd"), item(1, "mm"), p("Texto después")].concat();
    let text = all_text(&extract(&docx(&body, &[]), "docx").unwrap());
    assert!(text.contains("- ddmmaa\n  - dd\n  - mm\n\nTexto después"), "{text:?}");
}

// Falla 7: the same image used twice is one figure.
#[test]
fn an_image_used_twice_is_one_figure() {
    let rels = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/></Relationships>"#;
    let pic = r#"<w:p><w:r><w:drawing><a:blip r:embed="rId5"/></w:drawing></w:r></w:p>"#;
    let body = format!("{}{pic}{}{pic}", p("Uno"), p("Dos"));
    let doc = extract(&docx(&body, &[("word/_rels/document.xml.rels", rels), ("word/media/image1.png", b"PNG")]), "docx").unwrap();
    assert_eq!(doc.figures.len(), 1);
    assert_eq!(all_text(&doc).matches("[Figura F01").count(), 2, "referenced where it appears");
}

// Falla 9: the paragraphs of one text box stay together.
#[test]
fn the_lines_of_a_text_box_stay_together() {
    let bx = r#"<w:p><w:r><w:drawing><w:txbxContent><w:p><w:r><w:t>0000|</w:t></w:r></w:p><w:p><w:r><w:t>tipo  tasa</w:t></w:r></w:p></w:txbxContent></w:drawing></w:r></w:p>"#;
    let text = all_text(&extract(&docx(bx, &[]), "docx").unwrap());
    assert!(text.contains("0000|\ntipo  tasa"), "{text:?}");
}

// Falla 11: a long section is described in parts with their lines.
#[test]
fn a_long_section_comes_in_parts() {
    let t = tempfile::tempdir().unwrap();
    let body = std::iter::once(format!(r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Larga</w:t></w:r></w:p>"#))
        .chain((0..600).map(|n| p(&format!("Párrafo {n} con varias palabras para ocupar espacio en la sección larga del documento técnico."))))
        .collect::<String>();
    let src = t.path().join("largo.docx");
    std::fs::write(&src, docx(&body, &[])).unwrap();
    let built = build(&src, t.path(), &Options::default()).unwrap();
    let index: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(built.index).unwrap()).unwrap();
    let parts = index["sections"][0]["parts"].as_array().unwrap();
    assert!(parts.len() >= 3, "{parts:?}");
    assert_eq!(parts[0]["lines"][0], 1);
    let ends: Vec<u64> = parts.iter().map(|p| p["lines"][1].as_u64().unwrap()).collect();
    assert_eq!(ends.last().copied(), index["sections"][0]["lines"].as_u64(), "the parts cover the whole section");
}

// Falla 4: the estimate is calibrated on a measured technical document.
#[test]
fn the_token_estimate_counts_codes_and_symbols() {
    assert!(tokens_estimate("| Tipo registro | 2 | 1 | 2 | tmp_tipo_reg |") >= 15);
    assert!(tokens_estimate("hola") >= 1);
}

// Falla 10: a file that is not a whole workbook says so.
#[test]
fn an_incomplete_workbook_says_what_is_missing() {
    let mut out = std::io::Cursor::new(Vec::new());
    { let mut z = zip::ZipWriter::new(&mut out); z.start_file("xl/worksheets/sheet1.xml", zip::write::SimpleFileOptions::default()).unwrap(); z.write_all(b"<worksheet/>").unwrap(); z.finish().unwrap(); }
    let err = extract(&out.into_inner(), "xlsx").unwrap_err();
    assert!(err.contains("No es un libro de Excel completo") && err.contains("xl/workbook.xml"), "{err}");
}

//! Word pages come from a real layout engine of the machine (Word or
//! LibreOffice). Without one the test says so and passes, unless
//! DOCPACK_REQUIRE_LAYOUT is set (CI installs LibreOffice and sets it).
use docpack::{build, Options};
use std::io::Write;

/// A complete .docx (what Word and LibreOffice accept), with real heading styles.
fn full_docx(body: &str) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut out);
        let o = zip::write::SimpleFileOptions::default();
        let parts: [(&str, String); 4] = [
            ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#.into()),
            ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.into()),
            ("word/_rels/document.xml.rels", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#.into()),
            ("word/styles.xml", r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr></w:style></w:styles>"#.into()),
        ];
        for (name, xml) in parts { z.start_file(name, o).unwrap(); z.write_all(xml.as_bytes()).unwrap(); }
        z.start_file("word/document.xml", o).unwrap();
        z.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr/></w:body></w:document>"#).as_bytes()).unwrap();
        z.finish().unwrap();
    }
    out.into_inner()
}

fn heading(text: &str) -> String { format!(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#) }
fn para(text: &str) -> String { format!(r#"<w:p><w:r><w:t>{text}</w:t></w:r></w:p>"#) }
const PAGE_BREAK: &str = r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#;

#[test]
fn each_word_section_takes_the_page_of_its_heading() {
    let t = tempfile::tempdir().unwrap();
    let body = [heading("Alcance"), para("Texto del alcance."), PAGE_BREAK.into(), PAGE_BREAK.into(), heading("Validaciones"), para("Texto de las validaciones.")].concat();
    let src = t.path().join("diseno.docx");
    std::fs::write(&src, full_docx(&body)).unwrap();
    let built = build(&src, t.path(), &Options { pages: true, force: true }).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(built.pack.join("manifest.json")).unwrap()).unwrap();
    let notes = manifest["notes"].to_string();
    if notes.contains("Sin números de página") {
        assert!(std::env::var_os("DOCPACK_REQUIRE_LAYOUT").is_none(), "se pidió un motor de paginación y no hay: {notes}");
        eprintln!("sin Word ni LibreOffice en este equipo: {notes}");
        return;
    }
    let index: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&built.index).unwrap()).unwrap();
    let page = |title: &str| index["sections"].as_array().unwrap().iter().find(|s| s["title"] == title).map(|s| s["page_start"].clone());
    assert_eq!(page("Alcance"), Some(1.into()), "{index}");
    assert_eq!(page("Validaciones"), Some(3.into()), "{index}");
    assert_eq!(index["pages"], 3);
    // The page images come from the PDF the engine laid out: one per page,
    // unless this system has no renderer and the pack says so.
    if !notes.contains("Imágenes de página no generadas") {
        assert_eq!(built.page_images, 3, "{notes}");
    }
}

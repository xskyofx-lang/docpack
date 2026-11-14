//! The packs of a project together: catalog, search, comparing versions,
//! the health check and page text.
use docpack::{build, library, Options};
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name) }
fn json(path: &Path) -> Value { serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap() }
const QUICK: Options = Options { pages: false, force: false };

/// A Word document with a heading, a paragraph and one table.
fn docx(paragraph: &str, rows: &[[&str; 2]]) -> Vec<u8> {
    const W: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;
    let cell = |t: &str| format!("<w:tc><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc>");
    let table: String = rows.iter().map(|r| format!("<w:tr>{}{}</w:tr>", cell(r[0]), cell(r[1]))).collect();
    let body = format!(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Validaciones</w:t></w:r></w:p><w:p><w:r><w:t>{paragraph}</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Descripcin"/></w:pPr><w:r><w:t xml:space="preserve">Tabla </w:t></w:r><w:fldSimple w:instr=" SEQ Tabla \* ARABIC "><w:r><w:t>1</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> Códigos</w:t></w:r></w:p><w:tbl>{table}</w:tbl>"#);
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut out);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("word/document.xml", o).unwrap();
        z.write_all(format!(r#"<w:document {W}><w:body>{body}</w:body></w:document>"#).as_bytes()).unwrap();
        z.start_file("word/styles.xml", o).unwrap();
        z.write_all(format!(r#"<w:styles {W}><w:style w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#).as_bytes()).unwrap();
        z.start_file("docProps/core.xml", o).unwrap();
        z.write_all(r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/"><dc:title>Diseño de validaciones</dc:title><dcterms:modified>2026-10-28T10:00:00Z</dcterms:modified></cp:coreProperties>"#.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    out.into_inner()
}

/// A project with the two PDF fixtures built.
fn project() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    for name in ["sin_bordes.pdf", "tablas.pdf"] {
        std::fs::copy(fixture(name), t.path().join(name)).unwrap();
        build(&t.path().join(name), t.path(), &QUICK).unwrap();
    }
    t
}

#[test]
fn the_folder_of_packs_has_a_catalog() {
    let t = project();
    let catalog = json(&t.path().join("docpack").join("index.json"));
    let packs: Vec<&str> = catalog["packs"].as_array().unwrap().iter().map(|p| p["pack"].as_str().unwrap()).collect();
    assert_eq!(packs, ["sin_bordes.pdf", "tablas.pdf"]);
    assert_eq!(catalog["packs"][0]["pages"], 4);
    assert_eq!(catalog["packs"][0]["tables"], 3);
}

#[test]
fn tables_say_their_title_header_and_pages() {
    let t = project();
    let tables = json(&t.path().join("docpack/sin_bordes.pdf/tablas/index.json"));
    assert_eq!(tables[0]["title"], "Tabla 3: Ciclos de compensación");
    assert_eq!(tables[0]["header"], serde_json::json!(["Día", "Ciclo 1", "Ciclo 2", "Ciclo 3"]));
    assert_eq!(tables[0]["page"], 2);
    assert_eq!(tables[2]["pages"], serde_json::json!([3, 4]));
    assert!(tables[0].get("pages").is_none(), "one page: no list");
}

#[test]
fn without_page_images_only_the_pages_to_check_carry_theirs() {
    let t = project();
    let pack = t.path().join("docpack/sin_bordes.pdf");
    let manifest = json(&pack.join("manifest.json"));
    let notes = manifest["notes"].to_string();
    if notes.contains("Imágenes de página no generadas") {
        assert!(std::env::var_os("DOCPACK_REQUIRE_LAYOUT").is_none(), "se pidieron imágenes de página y no hubo: {notes}");
        eprintln!("sin motor de imágenes de página en este equipo: {notes}");
        return;
    }
    let images: Vec<String> = std::fs::read_dir(pack.join("paginas")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(images, ["p004.png"]);
    assert_eq!(json(&pack.join("index.json"))["page_images"]["only"], serde_json::json!([4]));
}

#[test]
fn find_goes_through_every_pack() {
    let t = project();
    let root = t.path().join("docpack");
    let found = library::find(&root, "ciclo 2", 10);
    let first = &found["hits"][0];
    assert_eq!((first["pack"].as_str(), first["table"].as_str(), first["row"].as_u64(), first["page"].as_u64()), (Some("sin_bordes.pdf"), Some("T01"), Some(1), Some(2)), "{found}");
    // A number is a whole number: "2" is not in "23:00".
    assert!(library::find(&root, "Campo 2", 50)["hits"].as_array().unwrap().iter().all(|h| !h["text"].as_str().unwrap().contains("Campo 20")), "Campo 2 is not Campo 20");
    // In text: its section, line and page.
    let text = library::find(&root, "acuse de recibo", 5);
    assert_eq!((text["hits"][0]["section"].as_str(), text["hits"][0]["page"].as_u64()), (Some("S02"), Some(2)), "{text}");
    // Accents and case do not matter; a table of contents comes last.
    let any = library::find(&root, "CONCILIACION", 50);
    assert!(any["total"].as_u64().unwrap() >= 2, "{any}");
    let toc = library::find(&root, "registro de detalle", 50);
    let hits = toc["hits"].as_array().unwrap();
    assert_eq!(hits.last().unwrap()["section"], "S01", "the contents' line is last: {toc}");
}

#[test]
fn two_versions_are_compared_by_section_and_table() {
    let t = tempfile::tempdir().unwrap();
    let (v1, v2) = (t.path().join("diseno_v1.3.docx"), t.path().join("diseno_v1.4.docx"));
    std::fs::write(&v1, docx("Se validan los campos.", &[["Código", "Mensaje"], ["E01", "Campo vacío"], ["E02", "Fecha inválida"]])).unwrap();
    std::fs::write(&v2, docx("Se validan los campos y la fecha.", &[["Código", "Mensaje"], ["E01", "Campo vacío"], ["E02", "Fecha fuera de rango"], ["E03", "Moneda inválida"]])).unwrap();
    let (a, b) = (build(&v1, t.path(), &QUICK).unwrap().pack, build(&v2, t.path(), &QUICK).unwrap().pack);
    let d = library::diff(&a, &b).unwrap();
    assert_eq!(d["same"], false);
    assert_eq!((d["a"]["version"].as_str(), d["b"]["version"].as_str()), (Some("1.3"), Some("1.4")), "versions from the file names");
    assert_eq!(d["b"]["date"], "2026-10-28", "date from the document's properties");
    assert_eq!(d["sections"]["changed"][0]["title"], "Validaciones");
    let table = &d["tables"]["changed"][0];
    assert_eq!(table["title"], "Tabla 1 Códigos", "a Word table's caption is its title");
    assert_eq!(table["removed"], serde_json::json!(["E02 | Fecha inválida"]));
    assert_eq!(table["added"], serde_json::json!(["E02 | Fecha fuera de rango", "E03 | Moneda inválida"]));
    assert_eq!(library::diff(&a, &a).unwrap()["same"], true);
}

#[test]
fn the_health_check_finds_what_is_wrong() {
    let t = project();
    let root = t.path().join("docpack");
    std::fs::write(root.join("log.txt"), "no es un pack").unwrap();
    std::fs::create_dir_all(root.join("a_medias.pdf")).unwrap();
    std::fs::create_dir_all(root.join(".x.pdf.tmp-1")).unwrap();
    std::fs::OpenOptions::new().append(true).open(t.path().join("tablas.pdf")).unwrap().write_all(b"\n%cambio").unwrap();
    std::fs::copy(fixture("diseno.pdf"), t.path().join("nuevo.pdf")).unwrap();
    std::fs::copy(fixture("diseno.pdf"), t.path().join("export_datos.pdf")).unwrap();
    std::fs::write(t.path().join(".docpackignore"), "# datos\nexport_*.pdf\n").unwrap();
    let report = library::doctor(t.path());
    let mut found: Vec<(String, String)> = report["list"].as_array().unwrap().iter()
        .map(|p| (p["problem"].as_str().unwrap().to_string(), Path::new(p["path"].as_str().unwrap()).file_name().unwrap().to_string_lossy().into_owned())).collect();
    found.sort();
    assert_eq!(found, [
        ("build_left_over".to_string(), ".x.pdf.tmp-1".to_string()),
        ("foreign_file".into(), "log.txt".into()),
        ("incomplete".into(), "a_medias.pdf".into()),
        ("no_pack".into(), "nuevo.pdf".into()),
        ("stale".into(), "tablas.pdf".into()),
    ], "{report}");
    assert!(library::ignored(t.path(), &t.path().join("export_datos.pdf")));
}

#[test]
fn the_text_of_some_pages() {
    let t = project();
    let text = library::page_text(&t.path().join("docpack/sin_bordes.pdf"), &library::parse_pages("2").unwrap()).unwrap();
    assert!(text.starts_with("<!-- página 2 -->") && text.contains("acuse de recibo"), "{text}");
    assert!(!text.contains("<!-- página 3"), "{text}");
    assert_eq!(library::parse_pages("4-5,9").unwrap(), [4, 5, 9]);
    assert!(library::parse_pages("5-4").is_err());
}

#[test]
fn find_filters_by_pack_by_kind_and_by_color() {
    let t = project();
    std::fs::copy(fixture("colores.pdf"), t.path().join("colores.pdf")).unwrap();
    build(&t.path().join("colores.pdf"), t.path(), &QUICK).unwrap();
    let root = t.path().join("docpack");
    let find = |query: &str, opts: library::Find| library::find_with(&root, query, &library::Find { limit: 50, ..opts });
    // "Only the pink ones": the cells in pink, with the pieces in pink, and nothing else.
    let pink = find("", library::Find { color: Some("rosa".into()), ..Default::default() });
    let hits = pink["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{pink}");
    assert_eq!((hits[0]["table"].as_str(), hits[0]["row"].as_u64(), hits[0]["colores"][0]["texto"][0].as_str()), (Some("T01"), Some(3), Some("Pendiente")));
    // Red: the vaccines in the table and the word in the text; by value as well as by name.
    let red = find("", library::Find { color: Some("FF0000".into()), ..Default::default() });
    let red: Vec<&Value> = red["hits"].as_array().unwrap().iter().collect();
    assert!(red.iter().any(|h| h["table"] == "T01" && h["colores"][0]["texto"][0].as_str().unwrap().starts_with("rabia, moquillo")), "{red:?}");
    assert!(red.iter().any(|h| h["section"].is_string() && h["text"].as_str().unwrap().contains("Los perros en rojo")), "the text without its tags: {red:?}");
    // Only tables, only one pack.
    let only = find("Perro", library::Find { only: Some("tablas".into()), pack: Some("colores".into()), ..Default::default() });
    assert!(only["hits"].as_array().unwrap().iter().all(|h| h["pack"] == "colores.pdf" && h["table"].is_string()), "{only}");
    assert!(find("Gato", library::Find { only: Some("texto".into()), ..Default::default() })["total"] == 0, "Gato is only in a table");
}

#[test]
fn find_names_a_table_by_its_title_and_counts_a_repeated_line_once() {
    let t = project();
    let root = t.path().join("docpack");
    let found = library::find(&root, "registro de detalle", 50);
    let first = &found["hits"][0];
    assert!(first["table"].is_string() && first["row"].is_null(), "a table found by its title comes first: {found}");
    // The running header of every page of a pack is one hit with its count.
    let header = library::find(&root, "Guía de prueba", 50);
    let in_sin_bordes: Vec<&Value> = header["hits"].as_array().unwrap().iter().filter(|h| h["pack"] == "sin_bordes.pdf").collect();
    assert!(in_sin_bordes.len() <= 1, "{header}");
}

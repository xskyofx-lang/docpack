//! Reading a pack by parts through DocPack itself, for an agent that has no
//! file reader of its own (an MCP-only client).
use docpack::{build, library, Options};
use std::path::Path;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn pack_of(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join(name);
    std::fs::copy(fixture(name), &src).unwrap();
    let pack = build(&src, t.path(), &Options { pages: false, force: false, in_project: false }).unwrap().pack;
    (t, pack)
}

/// Nothing named: the index, with the limits the manifest states.
#[test]
fn the_index_comes_with_the_notes_of_the_pack() {
    let (_t, pack) = pack_of("paginas_impresas.pdf");
    let index: serde_json::Value = serde_json::from_str(&library::read(&pack, &library::Read::default()).unwrap()).unwrap();
    assert_eq!(index["sections"].as_array().unwrap().len(), 6);
    assert!(index["notes"].to_string().contains("numeración impresa"), "{index}");
}

/// A section by id in any spelling or by a word of its title, with its pages
/// (the printed one too); a name that matches nothing lists what there is.
#[test]
fn a_section_by_id_or_title_says_where_it_comes_from() {
    let (_t, pack) = pack_of("paginas_impresas.pdf");
    let read = |name: &str| library::read(&pack, &library::Read { section: Some(name.into()), ..Default::default() });
    let by_id = read("s5").unwrap();
    assert!(by_id.starts_with("<!-- S05 · ") && by_id.contains("p. 5 (impresa 3)") && by_id.contains("reporte mensual"), "{by_id}");
    assert_eq!(read("disposiciones varias").unwrap(), by_id);
    let missing = read("S99").unwrap_err();
    assert!(missing.contains("S06 · Página 6"), "{missing}");
}

/// A table: its rows numbered as find numbers them, the header always, and
/// the colors of the rows shown.
#[test]
fn a_table_by_id_brings_its_header_rows_and_colors() {
    let (_t, pack) = pack_of("tablas.pdf");
    let read = |rows: Option<&str>| library::read(&pack, &library::Read { table: Some("T2".into()), rows: rows.map(str::to_string), ..Default::default() }).unwrap();
    let whole = read(None);
    assert!(whole.starts_with("<!-- T02 · ") && whole.contains("1: Código,Descripción") && whole.contains("3: E002,NOMBRE muy largo"), "{whole}");
    assert!(whole.contains("\"fondo\":\"DCDCDC (gris)\""), "{whole}");
    let some = read(Some("3"));
    assert!(some.contains("1: Código,Descripción") && some.contains("3: E002") && !some.contains("2: E001") && some.contains("se muestran 2"), "{some}");
}

/// A long section is read by parts: without one, the parts to pick; with
/// one, its lines and the page it starts on.
#[test]
fn a_long_section_is_read_by_parts() {
    let t = tempfile::tempdir().unwrap();
    let pack = t.path();
    std::fs::create_dir_all(pack.join("secciones")).unwrap();
    let body = "# Largo\n\n<!-- página 3 -->\nuno\ndos\n<!-- página 4 · impresa 2 -->\ntres\ncuatro\n";
    std::fs::write(pack.join("secciones/01-largo.md"), body).unwrap();
    std::fs::write(pack.join("index.json"), serde_json::json!({"sections": [{"id": "S01", "title": "Largo", "file": "secciones/01-largo.md", "page_start": 3, "page_end": 4, "tokens_estimate": 9000,
        "parts": [{"lines": [1, 5], "tokens_estimate": 4500, "starts": "Largo"}, {"lines": [7, 8], "tokens_estimate": 4500, "starts": "tres"}]}]}).to_string()).unwrap();
    let read = |part: Option<usize>| library::read(pack, &library::Read { section: Some("S01".into()), part, ..Default::default() });
    let list = read(None).unwrap();
    assert!(list.contains("pide una parte con part (1 a 2)") && list.contains("parte 2 · líneas 7–8") && !list.contains("cuatro"), "{list}");
    let second = read(Some(2)).unwrap();
    assert!(second.contains("parte 2 de 2") && second.contains("<!-- página 4 · impresa 2 -->\ntres\ncuatro"), "{second}");
    assert!(read(Some(3)).unwrap_err().contains("partes 1 a 2"));
}

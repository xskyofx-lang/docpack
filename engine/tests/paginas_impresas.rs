//! The page numbers a document prints, which it is quoted by ("Pág. 9"),
//! beside the PDF's own when they are not the same.
use docpack::{build, library, Options};
use serde_json::Value;
use std::path::Path;

fn pack(name: &str) -> (tempfile::TempDir, std::path::PathBuf, Value) {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join(name);
    std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name), &src).unwrap();
    let built = build(&src, t.path(), &Options { pages: false, force: false, in_project: false }).unwrap();
    let index = serde_json::from_str(&std::fs::read_to_string(&built.index).unwrap()).unwrap();
    (t, built.pack, index)
}

fn printed(index: &Value) -> Vec<(u64, Option<&str>)> {
    index["sections"].as_array().unwrap().iter().map(|s| (s["page_start"].as_u64().unwrap(), s["printed_start"].as_str())).collect()
}

/// The number in the footer that goes up page by page is the printed one; the
/// year beside it, the same on every page, is not; a cover without a number
/// has none, nor an index whose "ii" no other page goes on from.
#[test]
fn the_number_in_the_footer_is_the_page_the_document_is_quoted_by() {
    let (t, pack, index) = pack("paginas_impresas.pdf");
    assert_eq!(printed(&index), [(1, None), (2, None), (3, Some("1")), (4, Some("2")), (5, Some("3")), (6, Some("4"))]);
    let file = index["sections"][4]["file"].as_str().unwrap();
    let text = std::fs::read_to_string(pack.join(file)).unwrap();
    assert!(text.contains("_Página 5 · impresa 3_") && text.contains("<!-- página 5 · impresa 3 -->"), "{text}");
    let manifest = std::fs::read_to_string(pack.join("manifest.json")).unwrap();
    assert!(manifest.contains("la página 3 del PDF dice «1»"), "{manifest}");
    // A search names both pages, and asking for a page is by the PDF's.
    let found = library::find_with(&t.path().join("docpack"), "reporte mensual", &library::Find { limit: 5, pack: None, only: None, color: None });
    let hit = &found["hits"][0];
    assert_eq!((hit["page"].as_u64(), hit["printed_page"].as_str()), (Some(5), Some("3")), "{found}");
    let page = library::page_text(&pack, &[5]).unwrap();
    assert!(page.contains("reporte mensual"), "{page}");
}

/// Without a number on its pages, the numbering the PDF declares: Roman for
/// the front matter, then the annex's own with its prefix.
#[test]
fn without_a_printed_number_the_pdf_numbering_is_used() {
    let (_t, _, index) = pack("etiquetas.pdf");
    assert_eq!(printed(&index), [(1, Some("i")), (2, Some("ii")), (3, Some("A-1")), (4, Some("A-2")), (5, Some("A-3"))]);
}

/// A document whose footer number is the PDF's page carries nothing more.
#[test]
fn a_document_numbered_like_its_pdf_carries_no_printed_pages() {
    let (_t, pack, index) = pack("sin_bordes.pdf");
    assert!(printed(&index).iter().all(|(_, p)| p.is_none()), "{index}");
    assert!(!std::fs::read_to_string(pack.join("manifest.json")).unwrap().contains("numeración impresa"));
}

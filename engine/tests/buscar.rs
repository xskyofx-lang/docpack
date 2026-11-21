//! Searching the way an audit needs: several ways of saying one thing in one
//! search, and an empty search that says what it could not see.
use docpack::{build, library, Options};
use std::path::Path;

fn project() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("bases.pdf");
    std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/paginas_impresas.pdf"), &src).unwrap();
    build(&src, t.path(), &Options { pages: false, force: false, in_project: false }).unwrap();
    t
}

fn find(t: &tempfile::TempDir, query: &str) -> serde_json::Value {
    library::find_with(&t.path().join("docpack"), query, &library::Find { limit: 10, ..Default::default() })
}

/// "sanción | multa | penalidad": a place that says any of them is a hit,
/// and says which.
#[test]
fn several_ways_of_saying_it_in_one_search() {
    let t = project();
    let found = find(&t, "sanción | multa | penalidad");
    let hit = &found["hits"][0];
    assert_eq!(hit["matched"], "multa", "{found}");
    assert!(hit["text"].as_str().unwrap().contains("Multas"), "{found}");
    // One way alone names nothing more.
    assert!(find(&t, "multa")["hits"][0].get("matched").is_none());
}

/// Nothing found: the answer says it is no proof, which pages of the packs
/// may have incomplete text and which documents have no pack yet.
#[test]
fn an_empty_search_says_what_it_could_not_see() {
    let t = project();
    let found = find(&t, "garantía de fiel cumplimiento");
    assert_eq!(found["total"], 0);
    let note = found["not_found"]["note"].as_str().unwrap();
    assert!(note.contains("no prueba") && note.contains("otras formas de decirlo"), "{note}");
    assert_eq!(found["not_found"]["packs_searched"], 1);

    // A page read by OCR, and a document still without a pack.
    let manifest_path = t.path().join("docpack/bases.pdf/manifest.json");
    let mut manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["doubtful_pages"] = serde_json::json!({"6": "ocr"});
    std::fs::write(&manifest_path, manifest.to_string()).unwrap();
    std::fs::copy(t.path().join("bases.pdf"), t.path().join("circular.pdf")).unwrap();
    let found = find(&t, "garantía | boleta de garantía");
    let unseen = &found["not_found"];
    assert_eq!(unseen["doubtful_pages"]["bases.pdf"]["6"], "ocr", "{found}");
    assert_eq!(unseen["without_pack"][0], "circular.pdf", "{found}");
    let note = unseen["note"].as_str().unwrap();
    assert!(note.contains("doubtful_pages") && note.contains("without_pack") && !note.contains("otras formas"), "{note}");
}

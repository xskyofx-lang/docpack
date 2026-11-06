use docpack::{build, pack_dir, Options};
use std::path::Path;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// A PDF with bookmarks: one section per bookmark (levels 1-2), each covering
/// the pages up to the next one, with page markers in the text.
#[test]
fn pdf_sections_follow_its_bookmarks() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("diseno.pdf");
    std::fs::copy(fixture("diseno.pdf"), &src).unwrap();
    let built = build(&src, t.path(), &Options { pages: false, force: false }).unwrap();
    assert_eq!(built.status, "built");
    assert_eq!(built.pack, t.path().join("docpack").join("diseno.pdf"));
    let index: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&built.index).unwrap()).unwrap();
    let titles: Vec<&str> = index["sections"].as_array().unwrap().iter().map(|s| s["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["1. Objetivo", "2. Alcance", "3. Validaciones", "3.1 Campos", "3.2 Errores", "4. Flujo"]);
    let flujo = &index["sections"][5];
    assert_eq!((flujo["page_start"].as_u64(), flujo["page_end"].as_u64()), (Some(6), Some(7)), "a section runs to the next bookmark");
    let campos = std::fs::read_to_string(built.pack.join(index["sections"][3]["file"].as_str().unwrap())).unwrap();
    assert!(campos.contains("<!-- página 4 -->") && campos.contains("El campo ID es obligatorio"), "{campos}");
    // The same document again: the pack is current and is not rebuilt.
    assert_eq!(build(&src, t.path(), &Options::default()).unwrap().status, "up_to_date");
    // The original is untouched.
    assert_eq!(std::fs::read(&src).unwrap(), std::fs::read(fixture("diseno.pdf")).unwrap());
}

#[test]
fn a_file_outside_the_project_gets_its_pack_next_to_it() {
    let project = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let file = elsewhere.path().join("a.pdf");
    std::fs::write(&file, b"x").unwrap();
    assert_eq!(pack_dir(project.path(), &file), elsewhere.path().join("docpack").join("a.pdf"));
    let inside = project.path().join("sub").join("b.docx");
    std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
    std::fs::write(&inside, b"x").unwrap();
    assert_eq!(pack_dir(project.path(), &inside), project.path().join("docpack").join("b.docx"));
}

#[test]
fn a_folder_that_is_not_a_pack_is_never_overwritten() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("diseno.pdf");
    std::fs::copy(fixture("diseno.pdf"), &src).unwrap();
    let foreign = t.path().join("docpack").join("diseno.pdf");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("mio.txt"), "no tocar").unwrap();
    assert!(build(&src, t.path(), &Options::default()).is_err());
    assert_eq!(std::fs::read_to_string(foreign.join("mio.txt")).unwrap(), "no tocar");
}

/// A quick pack without pages (an attachment preview) is not reused when pages
/// are asked for later; a pack with pages serves a request without them.
#[test]
fn a_pack_without_pages_is_rebuilt_when_pages_are_asked() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("diseno.pdf");
    std::fs::copy(fixture("diseno.pdf"), &src).unwrap();
    let quick = Options { pages: false, force: false };
    let full = Options { pages: true, force: false };
    assert_eq!(build(&src, t.path(), &quick).unwrap().status, "built");
    assert_eq!(build(&src, t.path(), &quick).unwrap().status, "up_to_date");
    assert_eq!(build(&src, t.path(), &full).unwrap().status, "built", "pages were missing");
    assert_eq!(build(&src, t.path(), &full).unwrap().status, "up_to_date");
    assert_eq!(build(&src, t.path(), &quick).unwrap().status, "up_to_date", "a pack with pages also serves a quick read");
}

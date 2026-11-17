//! The documents in examples/ are the ones the README shows: everyday Word and
//! Excel files. These tests keep what the README says about them true.
use docpack::{build, library, Options};
use std::path::{Path, PathBuf};

fn example(name: &str) -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples").join(name) }

fn pack(name: &str, project: &Path) -> PathBuf {
    let built = build(&example(name), project, &Options { pages: false, force: true, in_project: true }).unwrap();
    built.pack
}

fn legend(pack: &Path) -> Vec<String> {
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("manifest.json")).unwrap()).unwrap();
    manifest["colores"].as_array().map(|a| a.iter().map(|c| format!("{} {}", c["uso"].as_str().unwrap(), c["color"].as_str().unwrap())).collect()).unwrap_or_default()
}

#[test]
fn the_word_plan_marks_pink_gray_and_red_but_not_its_blue_headings() {
    let dir = tempfile::tempdir().unwrap();
    let colors = legend(&pack("vaccination-plan-v1.1.docx", dir.path()));
    for expected in ["resaltado FF00FF", "fondo D9D9D9", "texto FF0000"] {
        assert!(colors.iter().any(|c| c == expected), "{expected} in {colors:?}");
    }
    // Word's own Heading 1 blue and the Title's navy are the document's style.
    assert!(!colors.iter().any(|c| c.ends_with("2F5496") || c.ends_with("1F3864")), "{colors:?}");
}

#[test]
fn the_pink_rows_are_found_by_their_color() {
    let dir = tempfile::tempdir().unwrap();
    pack("vaccination-plan-v1.1.docx", dir.path());
    let found = library::find_with(&dir.path().join("docpack"), "", &library::Find { limit: 50, pack: None, only: None, color: Some("rosa".into()) });
    let text = serde_json::to_string(&found).unwrap();
    assert!(text.contains("Parvovirus") && text.contains("Triple feline"), "{text}");
    assert!(!text.contains("Myxomatosis"), "the rabbit row is not pink: {text}");
}

#[test]
fn the_two_versions_differ_in_one_dose_one_row_and_two_sentences() {
    let dir = tempfile::tempdir().unwrap();
    let old = pack("vaccination-plan-v1.0.docx", dir.path());
    let new = pack("vaccination-plan-v1.1.docx", dir.path());
    let diff = serde_json::to_string(&library::diff(&old, &new).unwrap()).unwrap();
    assert!(diff.contains("1.5 ml") && diff.contains("Myxomatosis") && diff.contains("rabbits"), "{diff}");
}

#[test]
fn the_excel_inventory_loses_its_padding_and_keeps_formulas_and_colors() {
    let dir = tempfile::tempdir().unwrap();
    let pack = pack("clinic-inventory.xlsx", dir.path());
    let csv = std::fs::read_to_string(pack.join("tablas/T01.csv")).unwrap();
    assert!(csv.contains("Rabies vaccine,") && !csv.contains("Rabies vaccine  "), "padding gone: {csv}");
    let colors = legend(&pack);
    for expected in ["texto FF0000", "fondo D9D9D9"] {
        assert!(colors.iter().any(|c| c == expected), "{expected} in {colors:?}");
    }
    let manifest = std::fs::read_to_string(pack.join("manifest.json")).unwrap();
    assert!(manifest.contains("espacios de relleno"), "the manifest says the padding went");
    // The CSV shows the total; the formulas file says how it is calculated.
    assert!(csv.contains("Rabies vaccine,dose,120,9.5,1140"), "{csv}");
    let formulas: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("tablas/T01.formulas.json")).unwrap()).unwrap();
    let total = formulas.as_array().unwrap().iter().find(|f| f["fila"] == 3 && f["columna"] == 5).expect("the first total");
    assert_eq!(total["formula"], "=D4*E4");
    // The merged title above the table names it; the header is the real one.
    let tables: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("tablas/index.json")).unwrap()).unwrap();
    assert_eq!(tables[0]["header"][0], "Code");
    assert_eq!(tables[0]["header_row"], 2);
    assert_eq!(tables[0]["title"], "Inventory · Pet clinic - inventory, 4th quarter");
    assert_eq!(docpack::color::name("FFF2CC"), "amarillo", "Excel's light yellow");
    let summary: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("tablas/T02.formulas.json")).unwrap()).unwrap();
    assert!(summary.to_string().contains("SUMIF(Inventory!"), "{summary}");
}

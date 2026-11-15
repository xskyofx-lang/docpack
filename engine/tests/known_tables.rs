//! Tables someone checked by hand in real documents. The documents belong to
//! other people, so neither they nor what they say go in the repository: a
//! JSON file outside it lists them (DOCPACK_KNOWN_TABLES, else
//! ../pruebas-locales/tablas_conocidas.json when it exists). Without one the
//! test says so and passes.
//!
//! [{"file": "C:/…/guia.pdf", "page": 75, "title": "Table 10: …", "pages": [74, 75],
//!   "rows": 7, "columns": 7, "header": ["Business day", "…"], "cell": [1, 0, "Thursday"]}]
//! Every field but `file` and `page` is optional; `title` matches its start.
use docpack::{build, library, Options};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn known_tables_of_real_documents() {
    let list = std::env::var_os("DOCPACK_KNOWN_TABLES").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pruebas-locales/tablas_conocidas.json"));
    let Ok(text) = std::fs::read_to_string(&list) else { eprintln!("sin tablas conocidas en {}", list.display()); return };
    let known: Vec<Value> = serde_json::from_str(&text).expect("tablas_conocidas.json válido");
    // Each document is built once, on a copy in a temporary project: its own
    // folder is never written to.
    let work = tempfile::tempdir().unwrap();
    let mut packs: HashMap<String, PathBuf> = HashMap::new();
    let mut failures = Vec::new();
    for expected in &known {
        let file = expected["file"].as_str().expect("file");
        let pack = packs.entry(file.to_string()).or_insert_with(|| {
            let copy = work.path().join(PathBuf::from(file).file_name().unwrap());
            std::fs::copy(file, &copy).unwrap_or_else(|e| panic!("{file}: {e}"));
            build(&copy, work.path(), &Options { pages: false, force: false, in_project: false }).unwrap().pack
        }).clone();
        let tables: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(pack.join("tablas/index.json")).unwrap()).unwrap();
        let page = expected["page"].as_u64().expect("page");
        let title = expected["title"].as_str().unwrap_or("");
        let covers = |t: &Value| t["page"].as_u64() == Some(page) || t["pages"].as_array().is_some_and(|p| p.iter().any(|x| x.as_u64() == Some(page)));
        let Some(table) = tables.iter().find(|t| covers(t) && t["title"].as_str().unwrap_or("").starts_with(title)) else {
            failures.push(format!("{file} p.{page}: ninguna tabla «{title}…»; hay {:?}", tables.iter().filter(|t| covers(t)).map(|t| &t["title"]).collect::<Vec<_>>()));
            continue;
        };
        let rows = library::csv_rows(&std::fs::read_to_string(pack.join(table["file"].as_str().unwrap())).unwrap());
        let mut check = |what: &str, ok: bool, got: String| if !ok { failures.push(format!("{file} p.{page} {}: {what} es {got}", table["id"])) };
        if let Some(n) = expected["rows"].as_u64() { check("filas", rows.len() as u64 == n, rows.len().to_string()) }
        if let Some(n) = expected["columns"].as_u64() { check("columnas", table["columns"].as_u64() == Some(n), table["columns"].to_string()) }
        if let Some(h) = expected.get("header") { check("encabezado", &table["header"] == h, table["header"].to_string()) }
        if let Some(p) = expected.get("pages") { check("páginas", table.get("pages").unwrap_or(&serde_json::json!([table["page"]])) == p, table["pages"].to_string()) }
        if let Some(c) = expected["cell"].as_array() {
            let (r, k, want) = (c[0].as_u64().unwrap() as usize, c[1].as_u64().unwrap() as usize, c[2].as_str().unwrap());
            let got = rows.get(r).and_then(|row| row.get(k)).cloned().unwrap_or_default();
            check(&format!("celda [{r},{k}]"), got == want, format!("{got:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    eprintln!("{} tablas conocidas verificadas", known.len());
}

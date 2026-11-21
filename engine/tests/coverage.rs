//! A list of requirements checked against every pack in one pass: for each
//! requirement and part, where the documents say it, or that they do not
//! and what the search could not see.
use docpack::{build, library, Options};
use docpack::library::Requirement;
use std::path::Path;

#[test]
fn a_list_of_requirements_is_read_from_lines_or_json() {
    let text = "# bases\nReporte mensual = reporte mensual | informe mensual\n  de seguridad = seguridad\nMultas\n";
    let list = library::parse_requirements(text).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].name.as_str(), list[0].query.as_str()), ("Reporte mensual", "reporte mensual | informe mensual"));
    assert_eq!(list[0].parts, [Requirement { name: "de seguridad".into(), query: "seguridad".into(), parts: vec![] }]);
    assert_eq!((list[1].name.as_str(), list[1].query.as_str()), ("Multas", "Multas"), "a line without = names and searches itself");
    let json = r#"[{"name": "Multas", "query": "multa | sanción", "parts": [{"query": "por día"}]}]"#;
    let from_json = library::parse_requirements(json).unwrap();
    assert_eq!(from_json[0].parts[0].name, "por día");
    assert!(library::parse_requirements("  monto = monto").unwrap_err().contains("requisito arriba"));
    assert!(library::parse_requirements("Garantía =").unwrap_err().contains("Línea 1"));
}

#[test]
fn every_requirement_and_part_in_one_pass() {
    let t = tempfile::tempdir().unwrap();
    // The same rules twice: a requirement said in both documents counts both.
    for name in ["bases.pdf", "bases_v2.pdf"] {
        let src = t.path().join(name);
        std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/paginas_impresas.pdf"), &src).unwrap();
        build(&src, t.path(), &Options { pages: false, force: false, in_project: false }).unwrap();
    }
    let list = library::parse_requirements("Reporte mensual = reporte mensual | informe mensual\n  de seguridad = seguridad | prevención de riesgos\nGarantía de seriedad = garantía de seriedad | boleta de garantía\n").unwrap();
    let matrix = library::coverage(&t.path().join("docpack"), &list, &library::Find { limit: 1, ..Default::default() });
    let rows = matrix["requirements"].as_array().unwrap();
    let reporte = &rows[0];
    assert_eq!((reporte["found"].as_bool(), reporte["packs"].as_u64(), reporte["hits"].as_array().unwrap().len()), (Some(true), Some(2), 1), "{reporte}");
    let hit = &reporte["hits"][0];
    assert_eq!((hit["page"].as_u64(), hit["printed_page"].as_str(), hit["matched"].as_str()), (Some(5), Some("3"), Some("reporte mensual")), "{hit}");
    assert_eq!(reporte["parts"][0]["found"], true);
    assert_eq!(rows[1]["found"], false);
    assert_eq!(matrix["summary"], serde_json::json!({"asked": 3, "found": 2, "not_found": 1}));
    // What the searches could not see, once for the whole matrix.
    assert_eq!(matrix["not_found"]["packs_searched"], 2, "{matrix}");
    assert!(matrix["note"].as_str().unwrap().contains("no un veredicto"));
}

//! PDF tables come from their drawn borders (tests/fixtures/tablas_pdf.py).
use docpack::extract;

#[test]
fn bordered_pdf_tables_become_csv_and_leave_the_text() {
    let doc = extract(include_bytes!("fixtures/tablas.pdf"), "pdf").unwrap();
    assert_eq!(doc.tables.len(), 2, "{:?}", doc.tables);
    // Stroked borders; a cell with two lines keeps them.
    assert_eq!(doc.tables[0].rows, vec![
        vec!["Campo", "Longitud", "Reglas"],
        vec!["ID", "10", "Obligatorio\nNumérico"],
        vec!["NOMBRE", "40", "Máximo 40"],
    ]);
    // Thin filled borders (as Word exports them), a shaded header with no
    // border between its cells, continued on the next page under the same
    // running header, its repeated header row written once.
    assert_eq!(doc.tables[1].rows, vec![
        vec!["Código", "Descripción"],
        vec!["E001", "ID vacío"],
        vec!["E002", "NOMBRE muy largo"],
    ]);
    assert_eq!(doc.tables[1].page, Some(1));
    let text: String = doc.sections.iter().map(|s| s.text.as_str()).collect();
    assert!(!text.contains("Obligatorio") && !text.contains("E002"), "the table's text is in the CSV, not twice: {text}");
    assert_eq!(text.matches("[Tabla T02").count(), 2, "referenced on both pages it spans: {text}");
    assert!(text.contains("Fin del documento."));
}

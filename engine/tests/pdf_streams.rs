//! PDF tables without a full grid of borders (tests/fixtures/sin_bordes_pdf.py):
//! read by the blank lanes between their columns, by the rules between their
//! rows, or by rules open at the sides; continued across pages.
use docpack::extract;

fn doc() -> docpack::Document { extract(include_bytes!("fixtures/sin_bordes.pdf"), "pdf").unwrap() }

#[test]
fn a_table_without_borders_is_read_by_its_columns() {
    let doc = doc();
    assert_eq!(doc.tables.len(), 3, "{:#?}", doc.tables.iter().map(|t| &t.title).collect::<Vec<_>>());
    let t = &doc.tables[0];
    assert_eq!(t.title, "Tabla 3: Ciclos de compensación", "its caption, kept out of the rows");
    assert_eq!((t.page, t.pages.as_slice()), (Some(2), [2].as_slice()));
    assert_eq!(t.rows, vec![
        vec!["Día", "Ciclo 1", "Ciclo 2", "Ciclo 3"],
        vec!["Lunes", "Domingo\n23:00 UTC", "Lunes\n02:00 UTC", "Lunes\n05:00 UTC"],
        vec!["Martes", "Lunes\n23:00 UTC", "Martes\n02:00 UTC", "Martes\n05:00 UTC"],
        vec!["Miércoles", "Martes\n23:00 UTC", "Miércoles\n02:00 UTC", "Miércoles\n05:00 UTC"],
    ], "a shaded header without borders, cells of two lines");
}

#[test]
fn paragraphs_and_lists_are_not_tables() {
    let doc = doc();
    let page2 = &doc.sections[1].text;
    for words in ["Los ciclos se procesan en orden", "Cada ciclo genera su propio acuse", "El último ciclo del día"] {
        assert!(page2.contains(words), "{words:?} stays as text: {page2}");
    }
    assert!(!page2.contains("Domingo"), "the table's cells are in its CSV, not twice: {page2}");
    assert!(doc.tables.iter().all(|t| t.rows.iter().flatten().all(|c| !c.contains("Guía de prueba"))), "the running header is in no table");
}

#[test]
fn two_columns_with_rules_between_rows_are_a_table() {
    let t = &doc().tables[1];
    assert_eq!(t.title, "Tabla 4: Campos del registro");
    assert_eq!(t.rows, vec![
        vec!["Campo", "Descripción"],
        vec!["Código de transacción", "Identifica el tipo de mensaje."],
        vec!["Moneda", "Código ISO de tres letras."],
        vec!["Importe", "Importe en la moneda indicada."],
    ]);
}

#[test]
fn a_table_open_at_the_sides_goes_on_across_pages() {
    let t = &doc().tables[2];
    assert_eq!(t.title, "Tabla 5: Registro de detalle");
    assert_eq!(t.pages, vec![3, 4], "one table, both pages");
    assert_eq!(t.rows.len(), 24, "header once and 23 rows");
    assert_eq!(t.rows[0], vec!["Posición", "Longitud", "Contenido"]);
    assert_eq!(t.rows[1], vec!["1-4", "4", "Campo 1"]);
    assert_eq!(t.rows[23], vec!["89-92", "4", "Campo 23"]);
}

#[test]
fn what_looks_like_a_table_is_listed_to_check_with_its_image() {
    let doc = doc();
    assert_eq!(doc.check_pages, vec![4], "terms and meanings in two columns, without rules");
    let notes = doc.notes.join(" | ");
    assert!(notes.contains("alineación de sus columnas en las páginas 2–3"), "{notes}");
    assert!(notes.contains("La página 4 parece tener tablas sin bordes"), "{notes}");
    assert!(doc.sections[3].text.contains("Adquirente"), "it stays as text");
}

#[test]
fn a_table_of_contents_is_marked() {
    let doc = doc();
    let contents: Vec<bool> = doc.sections.iter().map(|s| s.contents).collect();
    assert_eq!(contents, [true, false, false, false]);
    assert!(doc.sections[0].tables.is_empty(), "its leaders are not columns");
}

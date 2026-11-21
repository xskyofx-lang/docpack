//! What a review of a workbook must know beyond the values it shows: sheets
//! hidden from whoever opens it, protection, cells with an error and numbers
//! typed by hand in a column calculated by formula, each at its cell ("D3").
use docpack::{build, extract, library, Options};
use std::io::Write;

/// A book with two sheets: "Costos" (protected) starting at column B, and
/// "Tarifas" (hidden). The structure of the book is protected too.
fn book() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut out);
        let o = zip::write::SimpleFileOptions::default();
        let mut put = |name: &str, body: &str| { z.start_file(name, o).unwrap(); z.write_all(body.as_bytes()).unwrap(); };
        put("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#);
        put("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#);
        put("xl/workbook.xml", r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookProtection lockStructure="1"/><sheets><sheet name="Costos" sheetId="1" r:id="rId1"/><sheet name="Tarifas" sheetId="2" state="hidden" r:id="rId2"/></sheets></workbook>"#);
        put("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#);
        // B1:E1 header; D (Total) = B*C, but D3 was typed over; E2 is broken.
        put("xl/worksheets/sheet1.xml", r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
<row r="1"><c r="B1" t="inlineStr"><is><t>Cantidad</t></is></c><c r="C1" t="inlineStr"><is><t>Precio</t></is></c><c r="D1" t="inlineStr"><is><t>Total</t></is></c><c r="E1" t="inlineStr"><is><t>Ajuste</t></is></c></row>
<row r="2"><c r="B2"><v>2</v></c><c r="C2"><v>10</v></c><c r="D2"><f>B2*C2</f><v>20</v></c><c r="E2" t="e"><f>#REF!*2</f><v>#REF!</v></c></row>
<row r="3"><c r="B3"><v>3</v></c><c r="C3"><v>10</v></c><c r="D3"><v>99</v></c></row>
<row r="4"><c r="B4"><v>4</v></c><c r="C4"><v>10</v></c><c r="D4"><f>B4*C4</f><v>40</v></c></row>
</sheetData><sheetProtection sheet="1" objects="1"/></worksheet>"#);
        put("xl/worksheets/sheet2.xml", r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Zona</t></is></c><c r="B1" t="inlineStr"><is><t>Tarifa</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>Norte</t></is></c><c r="B2"><v>12</v></c></row></sheetData></worksheet>"#);
        z.finish().unwrap();
    }
    out.into_inner()
}

#[test]
fn a_review_sees_hidden_sheets_protection_errors_and_typed_numbers() {
    let doc = extract(&book(), "xlsx").unwrap();
    let costos = &doc.tables[0];
    let sheet = costos.sheet.as_ref().unwrap();
    assert_eq!((sheet.name.as_str(), sheet.origin, sheet.protected, sheet.hidden), ("Costos", (0, 1), true, false));
    assert!(doc.tables[1].sheet.as_ref().unwrap().hidden, "Tarifas is hidden");
    let found: Vec<(&str, usize, usize, &str)> = costos.findings.iter().map(|f| (f.kind, f.row, f.column, f.value.as_str())).collect();
    assert_eq!(found, [("error", 1, 3, "#REF!"), ("constante", 2, 2, "99")]);
    let notes = doc.notes.join("\n");
    assert!(notes.contains("Hojas ocultas") && notes.contains("«Tarifas»"), "{notes}");
    assert!(notes.contains("protegida su estructura") && notes.contains("«Costos»"), "{notes}");
    assert!(notes.contains("1 con error y 1 con un número escrito a mano"), "{notes}");
}

/// In the pack each cell is named as the sheet names it, and a search hit in
/// a table says which cell holds the words.
#[test]
fn cells_are_named_as_the_sheet_names_them() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("costos.xlsx");
    std::fs::write(&src, book()).unwrap();
    let pack = build(&src, t.path(), &Options { pages: false, force: false, in_project: false }).unwrap().pack;
    let tables: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("tablas/index.json")).unwrap()).unwrap();
    let costos = &tables[0];
    assert_eq!((costos["sheet"].as_str(), costos["origin"].as_str(), costos["protected"].as_bool()), (Some("Costos"), Some("B1"), Some(true)), "{costos}");
    assert_eq!(costos["review"]["constante"], 1);
    assert_eq!(tables[1]["hidden"], true);
    let review = std::fs::read_to_string(pack.join(costos["review_file"].as_str().unwrap())).unwrap();
    assert!(review.contains("\"celda\":\"D3\"") && review.contains("\"celda\":\"E2\""), "{review}");
    let formulas = std::fs::read_to_string(pack.join(costos["formulas_file"].as_str().unwrap())).unwrap();
    assert!(formulas.contains("\"celda\":\"D2\""), "{formulas}");
    let found = library::find_with(&t.path().join("docpack"), "Norte", &library::Find { limit: 5, ..Default::default() });
    let hit = &found["hits"][0];
    assert_eq!((hit["sheet"].as_str(), hit["cell"].as_str(), hit["column"].as_u64()), (Some("Tarifas"), Some("A2"), Some(0)), "{found}");
}

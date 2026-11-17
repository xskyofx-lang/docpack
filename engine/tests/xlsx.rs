//! Excel as it is used every day: a fixed-width export that pads its cells,
//! rows filled in a theme color with a tint, an indexed red, a cell in green
//! text and a rich-text cell with one word in red.
use docpack::extract;
use std::io::Write;

/// A whole workbook (the parts Excel writes) with the given sheet, shared
/// strings and styles.
fn workbook(sheet: &str, shared: &str, styles: &str, theme: &str) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut out);
        let o = zip::write::SimpleFileOptions::default();
        let mut put = |name: &str, body: &str| { z.start_file(name, o).unwrap(); z.write_all(body.as_bytes()).unwrap(); };
        put("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>"#);
        put("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#);
        put("xl/workbook.xml", r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Vacunas" sheetId="1" r:id="rId1"/></sheets></workbook>"#);
        put("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#);
        put("xl/worksheets/sheet1.xml", sheet);
        put("xl/sharedStrings.xml", shared);
        put("xl/styles.xml", styles);
        put("xl/theme/theme1.xml", theme);
        z.finish().unwrap();
    }
    out.into_inner()
}

const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme></a:themeElements></a:theme>"#;

// Fonts: 0 body (theme 1 = dark), 1 green text. Fills: 0 none, 1 gray125
// (Excel's own), 2 accent 4 at 80 % lighter, 3 indexed red. Styles (cellXfs):
// 0 plain, 1 cream fill, 2 green text, 3 red fill.
const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><color theme="1"/><name val="Calibri"/></font><font><sz val="11"/><color rgb="FF00B050"/><name val="Calibri"/></font></fonts><fills count="4"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor theme="7" tint="0.79998168889431442"/><bgColor indexed="64"/></patternFill></fill><fill><patternFill patternType="solid"><fgColor indexed="10"/></patternFill></fill></fills><cellXfs count="4"><xf numFmtId="0" fontId="0" fillId="0"/><xf numFmtId="0" fontId="0" fillId="2" applyFill="1"/><xf numFmtId="0" fontId="1" fillId="0" applyFont="1"/><xf numFmtId="0" fontId="0" fillId="3" applyFill="1"/></cellXfs></styleSheet>"#;

// 0..3 headers and names padded to their column's width, 4 a rich text with
// "NO" in red, 5 "Pendiente", 6 "Estado".
const SHARED: &str = r#"<?xml version="1.0" encoding="UTF-8"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="7" uniqueCount="7"><si><t xml:space="preserve">Mascota             </t></si><si><t xml:space="preserve">Vacuna              </t></si><si><t xml:space="preserve">Firulais            </t></si><si><t xml:space="preserve">Michi               </t></si><si><r><t xml:space="preserve">Aplica: </t></r><r><rPr><color rgb="FFFF0000"/><sz val="11"/></rPr><t>NO</t></r></si><si><t>Pendiente</t></si><si><t>Estado</t></si></sst>"#;

const SHEET: &str = r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>6</v></c></row><row r="2"><c r="A2" s="1" t="s"><v>2</v></c><c r="B2" s="2" t="s"><v>5</v></c></row><row r="3"><c r="A3" t="s"><v>3</v></c><c r="B3" t="s"><v>4</v></c><c r="C3" s="3"/></row></sheetData></worksheet>"#;

#[test]
fn padding_after_the_text_goes_and_the_book_says_how_much() {
    let doc = extract(&workbook(SHEET, SHARED, STYLES, THEME), "xlsx").unwrap();
    let rows = &doc.tables[0].rows;
    assert_eq!(rows[0], ["Mascota", "Vacuna", "Estado"]);
    assert_eq!(rows[1][0], "Firulais", "the padding of a fixed-width export is gone");
    let note = doc.notes.iter().find(|n| n.contains("espacios de relleno")).expect("the manifest says it");
    assert!(note.contains("Se quitaron 54 espacios"), "{note}");
}

#[test]
fn excel_cells_keep_their_fill_their_text_color_and_colored_words() {
    let doc = extract(&workbook(SHEET, SHARED, STYLES, THEME), "xlsx").unwrap();
    let colors = &doc.tables[0].colors;
    let at = |row: usize, column: usize| colors.iter().find(|c| c.row == row && c.column == column);
    // Accent 4 (FFC000) at 80 % lighter, as Excel shows it.
    assert_eq!(at(1, 0).and_then(|c| c.fill.clone()).as_deref(), Some("FFF2CC"));
    // A whole cell in green text.
    let green = at(1, 1).expect("green cell");
    assert_eq!(green.text[0].color.as_deref(), Some("00B050"));
    assert_eq!(green.text[0].text, "Pendiente");
    // Only the red word of a rich-text cell.
    let rich = at(2, 1).expect("rich cell");
    assert_eq!(rich.text.len(), 1);
    assert_eq!((rich.text[0].text.as_str(), rich.text[0].color.as_deref()), ("NO", Some("FF0000")));
    // A cell with no value, only a red fill (indexed 10).
    assert_eq!(at(2, 2).and_then(|c| c.fill.clone()).as_deref(), Some("FF0000"));
    // The plain body text is not a mark.
    assert!(at(0, 0).is_none());
    let legend: Vec<_> = doc.colors.entries().iter().map(|(k, h, _)| format!("{k} {h}")).collect();
    assert!(legend.contains(&"fondo FFF2CC".to_string()) && legend.contains(&"texto FF0000".to_string()), "{legend:?}");
}

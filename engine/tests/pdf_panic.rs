//! A PDF whose font breaks the reader's expectations (a Type0 font with no
//! /DescendantFonts) used to panic and take the whole process down. Now the
//! page comes out empty and the note says what failed and where.
use docpack::extract;

fn broken_pdf() -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        { let c = "BT /F1 12 Tf 20 100 Td (Hola) Tj ET"; format!("<< /Length {} >>\nstream\n{c}\nendstream", c.len()) },
        "<< /Type /Font /Subtype /Type0 /BaseFont /Roto /Encoding /Identity-H >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 7 0 R /Resources << /Font << /F2 8 0 R >> >> >>".to_string(),
        { let c = "BT /F2 12 Tf 20 100 Td (Pagina sana) Tj ET"; format!("<< /Length {} >>\nstream\n{c}\nendstream", c.len()) },
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).bytes());
    for o in offsets { out.extend(format!("{o:010} 00000 n \n").bytes()) }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).bytes());
    out
}

#[test]
fn a_page_that_panics_the_reader_does_not_abort_the_document() {
    let doc = extract(&broken_pdf(), "pdf").expect("the document is still built");
    assert_eq!(doc.pages, Some(2));
    let text: String = doc.sections.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("Pagina sana"), "the healthy page keeps its text: {text}");
    assert!(doc.notes.iter().any(|n| n.contains("falló") && n.contains("1 (")), "{:?}", doc.notes);
}

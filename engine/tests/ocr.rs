//! A scanned page is read with the system's OCR (tests/fixtures/escaneado_pdf.py).
//! Without an OCR engine the test says so and passes, unless DOCPACK_REQUIRE_OCR
//! is set (CI installs one and sets it).
use docpack::extract;

#[test]
fn a_scanned_page_is_read_by_ocr() {
    let doc = extract(include_bytes!("fixtures/escaneado.pdf"), "pdf").unwrap();
    let notes = doc.notes.join(" | ");
    if doc.ocr.is_none() {
        assert!(std::env::var_os("DOCPACK_REQUIRE_OCR").is_none(), "se pidió OCR y no hubo: {notes}");
        assert!(notes.contains("es una imagen"), "the pack says the page is a picture: {notes}");
        eprintln!("sin OCR en este equipo: {notes}");
        return;
    }
    let text = &doc.sections[0].text;
    for words in ["ACTA DE ENTREGA", "Mesa de control", "12 de marzo de 2026"] {
        assert!(text.contains(words), "{words:?} not in {text:?}");
    }
    assert!(text.contains("texto por OCR"), "the page says its text came from OCR: {text}");
    assert!(notes.contains("puede tener errores"), "{notes}");
}

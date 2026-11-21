//! A scanned page is read with the system's OCR (tests/fixtures/escaneado_pdf.py).
//! Without an OCR engine the test says so and passes, unless DOCPACK_REQUIRE_OCR
//! is set (CI installs one and sets it).
use docpack::extract;

/// A scanned page whose picture is written inside the page's content (an
/// inline image, tests/fixtures/escaneo_en_linea_pdf.py) has no image object
/// to measure: it is read by OCR because it has no text, not because of how
/// its picture is drawn. Without OCR it is marked as a page without text,
/// never left blank without a word.
#[test]
fn a_page_without_text_is_read_by_ocr_however_its_picture_is_drawn() {
    let doc = extract(include_bytes!("fixtures/escaneo_en_linea.pdf"), "pdf").unwrap();
    if doc.ocr.is_none() {
        assert!(std::env::var_os("DOCPACK_REQUIRE_OCR").is_none(), "se pidió OCR y no hubo: {:?}", doc.notes);
        assert_eq!(doc.doubtful_pages.get(&1), Some(&"sin_texto"), "{:?}", doc.doubtful_pages);
        assert!(doc.notes.join(" ").contains("no se pudieron leer por imagen"), "{:?}", doc.notes);
        return;
    }
    let text = &doc.sections[0].text;
    // Words, not a whole line: an OCR engine may misread a one-letter word
    // (Vision on macOS read this page's "y" as a Cyrillic "у").
    for words in ["CONSTANCIA DE VISITA", "inspector", "oferente", "accesos a la obra"] {
        assert!(text.contains(words), "{words:?} not in {text:?}");
    }
    assert_eq!(doc.doubtful_pages.get(&1), Some(&"ocr"));
}

/// A page with its own text and a scanned certificate pasted in (a third of
/// the page, tests/fixtures/escaneo_parcial_pdf.py): its own text stays, the
/// certificate's lines are added by OCR, and the page is marked as read by
/// OCR. Without OCR the page is marked as a picture that was not read.
#[test]
fn a_scanned_picture_beside_the_text_is_read_too() {
    let doc = extract(include_bytes!("fixtures/escaneo_parcial.pdf"), "pdf").unwrap();
    let text = &doc.sections[0].text;
    assert!(text.contains("Anexo 4: experiencia del oferente"), "{text}");
    if doc.ocr.is_none() {
        assert!(std::env::var_os("DOCPACK_REQUIRE_OCR").is_none(), "se pidió OCR y no hubo: {:?}", doc.notes);
        assert_eq!(doc.doubtful_pages.get(&1), Some(&"imagen_sin_ocr"), "{:?}", doc.doubtful_pages);
        return;
    }
    assert!(text.contains("texto de las imágenes de esta página") && text.contains("CERTIFICADO DE EXPERIENCIA"), "{text}");
    assert_eq!(text.matches("Anexo 4").count(), 1, "the page's own lines are not repeated: {text}");
    assert_eq!(doc.doubtful_pages.get(&1), Some(&"ocr"));
}

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

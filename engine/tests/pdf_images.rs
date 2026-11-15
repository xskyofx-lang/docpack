//! A PDF that paints an image keeps its text: the image's bytes are pixels,
//! never drawing operators, even when they happen to read like some.
use docpack::{build, Options};
use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};

fn pdf_with_image(pixels: &[u8]) -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let image_id = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8 },
        pixels.to_vec(),
    ));
    let content = Content { operations: vec![
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec!["F1".into(), 12.into()]),
        Operation::new("Td", vec![72.into(), 720.into()]),
        Operation::new("Tj", vec![Object::string_literal("Perro vacunado")]),
        Operation::new("ET", vec![]),
        Operation::new("q", vec![]),
        Operation::new("cm", vec![10.into(), 0.into(), 0.into(), 10.into(), 72.into(), 600.into()]),
        Operation::new("Do", vec!["Im1".into()]),
        Operation::new("Q", vec![]),
    ] };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id }, "XObject" => dictionary! { "Im1" => image_id } },
    });
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1 }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[test]
fn image_bytes_that_read_like_operators_do_not_break_the_text() {
    let t = tempfile::tempdir().unwrap();
    let src = t.path().join("mascotas.pdf");
    // "c" alone would be a curve with no coordinates if read as a content stream.
    std::fs::write(&src, pdf_with_image(b"c")).unwrap();
    let built = build(&src, t.path(), &Options { pages: false, force: false, in_project: false }).unwrap();
    let mut text = String::new();
    for entry in std::fs::read_dir(built.pack.join("secciones")).unwrap() {
        text += &std::fs::read_to_string(entry.unwrap().path()).unwrap();
    }
    assert!(text.contains("Perro vacunado"), "{text}");
}

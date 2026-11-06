//! PDF page images, rendered by the PDF engine that ships with Windows
//! (Windows.Data.Pdf): no bundled renderer, no network. Other systems get the
//! text pack without images, and the manifest says so.

/// Width of a page image: readable text in a vision model without large files.
#[cfg(windows)]
const PAGE_WIDTH: u32 = 1400;

#[cfg(windows)]
pub fn pdf_pages(bytes: &[u8], out: &std::path::Path) -> Result<Vec<String>, String> {
    use windows::{Data::Pdf::{PdfDocument, PdfPageRenderOptions}, Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream}};
    let e = |e: windows::core::Error| format!("Motor PDF de Windows: {}", e.message());
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let input = InMemoryRandomAccessStream::new().map_err(e)?;
    let writer = DataWriter::CreateDataWriter(&input).map_err(e)?;
    writer.WriteBytes(bytes).map_err(e)?;
    writer.StoreAsync().map_err(e)?.join().map_err(e)?;
    writer.FlushAsync().map_err(e)?.join().map_err(e)?;
    writer.DetachStream().map_err(e)?;
    input.Seek(0).map_err(e)?;
    let doc = PdfDocument::LoadFromStreamAsync(&input).map_err(e)?.join().map_err(|_| "PDF dañado, cifrado o no compatible con el motor de Windows".to_string())?;
    let total = doc.PageCount().map_err(e)?;
    let options = PdfPageRenderOptions::new().map_err(e)?;
    options.SetDestinationWidth(PAGE_WIDTH).map_err(e)?;
    let mut files = Vec::new();
    for i in 0..total {
        let page = doc.GetPage(i).map_err(e)?;
        let stream = InMemoryRandomAccessStream::new().map_err(e)?;
        page.RenderWithOptionsToStreamAsync(&stream, &options).map_err(e)?.join().map_err(e)?;
        let size = stream.Size().map_err(e)? as u32;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).map_err(e)?).map_err(e)?;
        reader.LoadAsync(size).map_err(e)?.join().map_err(e)?;
        let mut png = vec![0u8; size as usize];
        reader.ReadBytes(&mut png).map_err(e)?;
        let name = format!("paginas/p{:03}.png", i + 1);
        std::fs::write(out.join(format!("p{:03}.png", i + 1)), png).map_err(|e| e.to_string())?;
        files.push(name);
    }
    Ok(files)
}

#[cfg(not(windows))]
pub fn pdf_pages(_bytes: &[u8], _out: &std::path::Path) -> Result<Vec<String>, String> {
    Err("las imágenes de página usan el motor PDF de Windows; en este sistema el pack trae solo texto".into())
}

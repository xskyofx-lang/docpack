//! Page images and OCR, with what each system already has: nothing is
//! bundled and nothing goes to the network.
//! - Windows: its PDF engine (Windows.Data.Pdf) and its OCR (Windows.Media.Ocr),
//!   in the languages of the person's profile.
//! - macOS: Core Graphics draws the page and Vision reads it.
//! - Linux and others: `pdftoppm` (poppler) and `tesseract` when installed,
//!   with every language Tesseract has.
//!
//! Without them the pack keeps its text and the manifest says what is missing.

/// Width of a page image: readable text in a vision model without large files.
pub const PAGE_WIDTH: u32 = 1400;
/// Resolution OCR reads a page at; the image is never larger than the engine takes.
pub const OCR_DPI: f64 = 300.0;

/// Text read from page images: the engine (with its languages) and, per page
/// number, its lines.
pub struct Ocr { pub engine: String, pub pages: Vec<(u32, String)> }

pub use engine::{ocr, pdf_pages};

#[cfg(windows)]
mod engine {
    use super::{Ocr, OCR_DPI, PAGE_WIDTH};
    use windows::{
        Data::Pdf::{PdfDocument, PdfPageRenderOptions},
        Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat},
        Media::Ocr::OcrEngine,
        Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream},
    };
    fn e(e: windows::core::Error) -> String { format!("Windows: {}", e.message()) }

    fn load(bytes: &[u8]) -> Result<PdfDocument, String> {
        let input = InMemoryRandomAccessStream::new().map_err(e)?;
        let writer = DataWriter::CreateDataWriter(&input).map_err(e)?;
        writer.WriteBytes(bytes).map_err(e)?;
        writer.StoreAsync().map_err(e)?.join().map_err(e)?;
        writer.FlushAsync().map_err(e)?.join().map_err(e)?;
        writer.DetachStream().map_err(e)?;
        input.Seek(0).map_err(e)?;
        PdfDocument::LoadFromStreamAsync(&input).map_err(e)?.join().map_err(|_| "PDF dañado, cifrado o no compatible con el motor de Windows".to_string())
    }

    /// Page `index` (from 0) drawn as PNG `width` pixels wide.
    fn draw(doc: &PdfDocument, index: u32, width: u32) -> Result<InMemoryRandomAccessStream, String> {
        let page = doc.GetPage(index).map_err(e)?;
        let options = PdfPageRenderOptions::new().map_err(e)?;
        options.SetDestinationWidth(width).map_err(e)?;
        let stream = InMemoryRandomAccessStream::new().map_err(e)?;
        page.RenderWithOptionsToStreamAsync(&stream, &options).map_err(e)?.join().map_err(e)?;
        Ok(stream)
    }

    pub fn pdf_pages(bytes: &[u8], out: &std::path::Path) -> Result<Vec<String>, String> {
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let doc = load(bytes)?;
        let mut files = Vec::new();
        for i in 0..doc.PageCount().map_err(e)? {
            let stream = draw(&doc, i, PAGE_WIDTH)?;
            let size = stream.Size().map_err(e)? as u32;
            let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).map_err(e)?).map_err(e)?;
            reader.LoadAsync(size).map_err(e)?.join().map_err(e)?;
            let mut png = vec![0u8; size as usize];
            reader.ReadBytes(&mut png).map_err(e)?;
            std::fs::write(out.join(format!("p{:03}.png", i + 1)), png).map_err(|e| e.to_string())?;
            files.push(format!("paginas/p{:03}.png", i + 1));
        }
        Ok(files)
    }

    pub fn ocr(bytes: &[u8], pages: &[u32]) -> Result<Ocr, String> {
        let engine = OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| "Windows no tiene OCR para los idiomas de este perfil".to_string())?;
        let language = engine.RecognizerLanguage().and_then(|l| l.LanguageTag()).map(|t| t.to_string()).unwrap_or_default();
        let largest = OcrEngine::MaxImageDimension().map_err(e)? as f64;
        let doc = load(bytes)?;
        let mut out = Vec::new();
        for &n in pages {
            let size = doc.GetPage(n - 1).and_then(|p| p.Size()).map_err(e)?;
            // Page size in DIPs (1/96 inch): OCR_DPI, within what the engine takes.
            let scale = (OCR_DPI / 96.0).min(largest / (size.Width.max(size.Height) as f64).max(1.0));
            let stream = draw(&doc, n - 1, (size.Width as f64 * scale) as u32)?;
            stream.Seek(0).map_err(e)?;
            let decoder = BitmapDecoder::CreateAsync(&stream).map_err(e)?.join().map_err(e)?;
            let bitmap = decoder.GetSoftwareBitmapConvertedAsync(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Premultiplied).map_err(e)?.join().map_err(e)?;
            let result = engine.RecognizeAsync(&bitmap).map_err(e)?.join().map_err(e)?;
            let lines: Vec<String> = result.Lines().map_err(e)?.into_iter().filter_map(|l| l.Text().ok()).map(|t| t.to_string()).collect();
            out.push((n, lines.join("\n")));
        }
        Ok(Ocr { engine: format!("OCR de Windows ({language})"), pages: out })
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod engine {
    use super::{Ocr, OCR_DPI, PAGE_WIDTH};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn which(name: &str) -> Option<PathBuf> {
        std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|f| f.is_file()))
    }

    /// A private folder with the PDF in it, removed when dropped.
    struct Work(PathBuf);
    impl Work {
        fn new(bytes: &[u8]) -> Result<Self, String> {
            let dir = std::env::temp_dir().join(format!("docpack-{}-{}", std::process::id(), crate::sha256(bytes).get(..12).unwrap_or("pdf")));
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("in.pdf"), bytes).map_err(|e| e.to_string())?;
            Ok(Work(dir))
        }
    }
    impl Drop for Work { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

    fn run(cmd: &mut Command) -> Result<Vec<u8>, String> {
        let out = cmd.output().map_err(|e| e.to_string())?;
        if out.status.success() { Ok(out.stdout) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
    }

    /// The PNG pdftoppm wrote for `prefix` (it pads the page number to the page count).
    fn written(dir: &Path, prefix: &str) -> Option<PathBuf> {
        std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(prefix) && n.ends_with(".png")))
    }

    pub fn pdf_pages(bytes: &[u8], out: &Path) -> Result<Vec<String>, String> {
        let pdftoppm = which("pdftoppm").ok_or("las imágenes de página usan pdftoppm (poppler-utils), que no está instalado")?;
        let work = Work::new(bytes)?;
        run(Command::new(&pdftoppm).args(["-png", "-scale-to-x", &PAGE_WIDTH.to_string(), "-scale-to-y", "-1"]).arg(work.0.join("in.pdf")).arg(work.0.join("p")))?;
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let mut pages: Vec<(u32, PathBuf)> = std::fs::read_dir(&work.0).map_err(|e| e.to_string())?.flatten().map(|e| e.path())
            .filter_map(|p| { let n = p.file_stem()?.to_str()?.strip_prefix("p-")?.parse().ok()?; Some((n, p)) }).collect();
        pages.sort();
        let mut files = Vec::new();
        for (n, file) in pages {
            std::fs::copy(&file, out.join(format!("p{n:03}.png"))).map_err(|e| e.to_string())?;
            files.push(format!("paginas/p{n:03}.png"));
        }
        Ok(files)
    }

    pub fn ocr(bytes: &[u8], pages: &[u32]) -> Result<Ocr, String> {
        let pdftoppm = which("pdftoppm").ok_or("pdftoppm (poppler-utils) no está instalado")?;
        let tesseract = which("tesseract").ok_or("tesseract no está instalado")?;
        let listed = String::from_utf8_lossy(&run(Command::new(&tesseract).arg("--list-langs"))?).into_owned();
        // Every language installed, except the page-layout and math models.
        let languages: Vec<&str> = listed.lines().skip(1).map(str::trim).filter(|l| !l.is_empty() && !matches!(*l, "osd" | "equ")).collect();
        if languages.is_empty() { return Err("tesseract no tiene idiomas instalados".into()) }
        let languages = languages.join("+");
        let work = Work::new(bytes)?;
        let mut out = Vec::new();
        for &n in pages {
            let prefix = format!("o{n}");
            run(Command::new(&pdftoppm).args(["-png", "-r", &OCR_DPI.to_string(), "-f", &n.to_string(), "-l", &n.to_string()]).arg(work.0.join("in.pdf")).arg(work.0.join(&prefix)))?;
            let image = written(&work.0, &format!("{prefix}-")).ok_or("pdftoppm no dejó la imagen")?;
            let text = run(Command::new(&tesseract).arg(&image).arg("stdout").args(["-l", &languages]))?;
            out.push((n, String::from_utf8_lossy(&text).trim().to_string()));
        }
        Ok(Ocr { engine: format!("Tesseract ({languages})"), pages: out })
    }
}

#[cfg(target_os = "macos")]
mod engine {
    use super::{Ocr, OCR_DPI, PAGE_WIDTH};
    use objc2::{rc::Retained, AllocAnyThread};
    use objc2_core_foundation::{CFData, CFMutableData, CFRetained, CFString, CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{CGBitmapContextCreate, CGBitmapContextCreateImage, CGColorSpace, CGContext, CGDataProvider, CGImage, CGImageAlphaInfo, CGPDFBox, CGPDFDocument, CGPDFPage};
    use objc2_foundation::{NSArray, NSDictionary, NSLocale, NSString};
    use objc2_image_io::CGImageDestination;
    use objc2_vision::{VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel};

    fn document(bytes: &[u8]) -> Result<CFRetained<CGPDFDocument>, String> {
        let data = CFData::from_bytes(bytes);
        let provider = CGDataProvider::with_cf_data(Some(&data)).ok_or("Core Graphics no pudo leer el PDF")?;
        CGPDFDocument::with_provider(Some(&provider)).ok_or_else(|| "PDF dañado, cifrado o no compatible con Core Graphics".into())
    }

    /// Page `number` (from 1) drawn on white, `scale` pixels per point.
    fn draw(doc: &CGPDFDocument, number: usize, scale: f64) -> Result<CFRetained<CGImage>, String> {
        let page = CGPDFDocument::page(Some(doc), number).ok_or("página inexistente")?;
        let rect = CGPDFPage::box_rect(Some(&page), CGPDFBox::MediaBox);
        let (w, h) = ((rect.size.width * scale).ceil() as usize, (rect.size.height * scale).ceil() as usize);
        let space = CGColorSpace::new_device_rgb().ok_or("sin espacio de color")?;
        let ctx = unsafe { CGBitmapContextCreate(std::ptr::null_mut(), w, h, 8, 0, Some(&space), CGImageAlphaInfo::NoneSkipLast.0) }.ok_or("sin memoria para la imagen")?;
        CGContext::set_rgb_fill_color(Some(&ctx), 1.0, 1.0, 1.0, 1.0);
        CGContext::fill_rect(Some(&ctx), CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w as f64, h as f64)));
        CGContext::scale_ctm(Some(&ctx), scale, scale);
        CGContext::translate_ctm(Some(&ctx), -rect.origin.x, -rect.origin.y);
        CGContext::draw_pdf_page(Some(&ctx), Some(&page));
        CGBitmapContextCreateImage(Some(&ctx)).ok_or_else(|| "Core Graphics no dibujó la página".into())
    }

    fn png(image: &CGImage) -> Result<Vec<u8>, String> {
        let data = CFMutableData::new(None, 0).ok_or("sin memoria")?;
        let kind = CFString::from_static_str("public.png");
        let destination = unsafe { CGImageDestination::with_data(&data, &kind, 1, None) }.ok_or("ImageIO no escribe PNG")?;
        unsafe { destination.add_image(image, None) };
        if !unsafe { destination.finalize() } { return Err("ImageIO no terminó el PNG".into()) }
        Ok(data.to_vec())
    }

    pub fn pdf_pages(bytes: &[u8], out: &std::path::Path) -> Result<Vec<String>, String> {
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let doc = document(bytes)?;
        let mut files = Vec::new();
        for n in 1..=CGPDFDocument::number_of_pages(Some(&doc)) {
            let page = CGPDFDocument::page(Some(&doc), n).ok_or("página inexistente")?;
            let width = CGPDFPage::box_rect(Some(&page), CGPDFBox::MediaBox).size.width.max(1.0);
            let image = draw(&doc, n, PAGE_WIDTH as f64 / width)?;
            std::fs::write(out.join(format!("p{n:03}.png")), png(&image)?).map_err(|e| e.to_string())?;
            files.push(format!("paginas/p{n:03}.png"));
        }
        Ok(files)
    }

    /// The languages Vision reads that the person uses (System Settings order).
    fn languages(request: &VNRecognizeTextRequest) -> Vec<String> {
        let supported: Vec<String> = unsafe { request.supportedRecognitionLanguagesAndReturnError() }.map(|a| a.iter().map(|s| s.to_string()).collect()).unwrap_or_default();
        let base = |tag: &str| tag.split(['-', '_']).next().unwrap_or("").to_lowercase();
        let mut chosen: Vec<String> = Vec::new();
        for preferred in NSLocale::preferredLanguages().iter().map(|s| s.to_string()) {
            if let Some(found) = supported.iter().find(|s| base(s) == base(&preferred)) { if !chosen.contains(found) { chosen.push(found.clone()) } }
        }
        chosen
    }

    pub fn ocr(bytes: &[u8], pages: &[u32]) -> Result<Ocr, String> {
        let doc = document(bytes)?;
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        let chosen = languages(&request);
        if !chosen.is_empty() {
            let names: Vec<Retained<NSString>> = chosen.iter().map(|l| NSString::from_str(l)).collect();
            request.setRecognitionLanguages(&NSArray::from_retained_slice(&names));
        }
        let mut out = Vec::new();
        for &n in pages {
            let image = draw(&doc, n as usize, OCR_DPI / 72.0)?;
            let handler = unsafe { VNImageRequestHandler::initWithCGImage_options(VNImageRequestHandler::alloc(), &image, &NSDictionary::new()) };
            let as_request: &VNRequest = &request;
            handler.performRequests_error(&NSArray::from_slice(&[as_request])).map_err(|e| format!("Vision: {}", e.localizedDescription()))?;
            // Vision measures from the bottom left: top to bottom, then left to right.
            let mut lines: Vec<(f64, f64, String)> = request.results().map(|r| r.iter().filter_map(|o| {
                let text = o.topCandidates(1).firstObject()?.string().to_string();
                let bounds = unsafe { o.boundingBox() };
                Some((-(bounds.origin.y + bounds.size.height), bounds.origin.x, text))
            }).collect()).unwrap_or_default();
            lines.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            out.push((n, lines.into_iter().map(|l| l.2).collect::<Vec<_>>().join("\n")));
        }
        let engine = if chosen.is_empty() { "Vision de macOS".to_string() } else { format!("Vision de macOS ({})", chosen.join(", ")) };
        Ok(Ocr { engine, pages: out })
    }
}

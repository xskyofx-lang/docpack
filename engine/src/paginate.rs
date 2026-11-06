//! Page numbers for formats that do not store them (Word). A DOCX keeps no
//! pages: they exist only once a layout engine lays the text out, and
//! `lastRenderedPageBreak` is a stale hint (on real documents it gave 44, 101
//! and 40 pages for 43, 108 and 43). So the document is laid out for real by
//! an engine installed on this machine (Microsoft Word, else LibreOffice),
//! exported to PDF with one bookmark per heading, and each section takes the
//! page of its heading. Always on a copy in a temporary folder.
use crate::Document;
use std::path::{Path, PathBuf};

/// A document laid out as PDF, and the engine that did it.
pub struct Laid {
    pub pdf: Vec<u8>,
    pub engine: String,
}

/// Lays `bytes` (a .docx/.docm) out with the first engine this machine has.
pub fn layout(bytes: &[u8], ext: &str) -> Result<Laid, String> {
    let dir = std::env::temp_dir().join(format!("docpack-{}-{}", std::process::id(), crate::sha256(bytes).get(..12).unwrap_or("doc")));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let source = dir.join(format!("documento.{ext}"));
    let pdf = dir.join("documento.pdf");
    let result = (|| {
        std::fs::write(&source, bytes).map_err(|e| e.to_string())?;
        let mut tried = Vec::new();
        #[cfg(windows)]
        match word::export(&source, &pdf) {
            Ok(engine) => return read(&pdf, engine),
            Err(e) => tried.push(format!("Word: {e}")),
        }
        match libreoffice(&source, &dir) {
            Ok(engine) => return read(&pdf, engine),
            Err(e) => tried.push(format!("LibreOffice: {e}")),
        }
        Err(tried.join("; "))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn read(pdf: &Path, engine: String) -> Result<Laid, String> {
    let pdf = std::fs::read(pdf).map_err(|e| format!("{engine} no dejó el PDF: {e}"))?;
    Ok(Laid { pdf, engine })
}

/// Puts each section on the page of its heading in the laid-out PDF. Headings
/// are matched in order against the PDF's bookmarks, so a title repeated in a
/// table of contents or later in the text cannot pull a section elsewhere.
pub fn assign_pages(doc: &mut Document, laid: &Laid) {
    let Ok(pdf) = lopdf::Document::load_mem(&laid.pdf) else { doc.notes.push(format!("{} exportó un PDF ilegible: sin números de página.", laid.engine)); return };
    let total = pdf.get_pages().len() as u32;
    let marks: Vec<(String, u32)> = pdf.get_toc().map(|toc| toc.toc.into_iter().filter(|t| t.page >= 1).map(|t| (normal(&t.title), t.page as u32)).collect()).unwrap_or_default();
    doc.pages = Some(total);
    // Headers and footers repeat on every page: they have no page of their own.
    let is_heading = |s: &crate::Section| s.level >= 1 && !s.every_page;
    let headings = doc.sections.iter().filter(|s| is_heading(s)).count();
    let mut cursor = 0;
    for section in doc.sections.iter_mut() {
        if section.level == 0 { section.page_start = Some(1); continue }
        if !is_heading(section) { continue }
        let title = normal(&section.title);
        if title.is_empty() { continue }
        if let Some(at) = marks[cursor..].iter().position(|(m, _)| same_title(m, &title)) {
            section.page_start = Some(marks[cursor + at].1);
            cursor += at + 1;
        }
    }
    // A heading Word gave no bookmark (a "Contents" title, a heading by its own
    // outline level) is looked for in the text, only on the pages between the
    // located headings around it: never in a table of contents elsewhere.
    if doc.sections.iter().any(|s| is_heading(s) && s.page_start.is_none()) {
        let pages: Vec<Vec<String>> = pdf_extract::extract_text_from_mem_by_pages(&laid.pdf).unwrap_or_default()
            .iter().map(|p| p.lines().map(normal).filter(|l| !l.is_empty()).collect()).collect();
        let starts: Vec<Option<u32>> = doc.sections.iter().map(|s| s.page_start).collect();
        for i in 0..doc.sections.len() {
            if !is_heading(&doc.sections[i]) || doc.sections[i].page_start.is_some() { continue }
            let lo = starts[..i].iter().rev().flatten().next().copied().unwrap_or(1);
            let hi = starts[i + 1..].iter().flatten().next().copied().unwrap_or(total);
            let title = normal(&doc.sections[i].title);
            let hit = (lo..=hi).find(|p| pages.get(*p as usize - 1).is_some_and(|lines| lines.iter().any(|l| same_title(l, &title))));
            doc.sections[i].page_start = hit;
        }
    }
    // A section runs to the page where the next located one starts.
    let starts: Vec<Option<u32>> = doc.sections.iter().map(|s| s.page_start).collect();
    for (i, section) in doc.sections.iter_mut().enumerate() {
        let Some(start) = section.page_start else { continue };
        let next = starts[i + 1..].iter().flatten().next().copied();
        section.page_end = Some(next.map(|n| n.max(start)).unwrap_or(total).max(start));
    }
    let found = doc.sections.iter().filter(|s| is_heading(s) && s.page_start.is_some()).count();
    doc.notes.retain(|n| !n.starts_with("Word no guarda páginas"));
    if found == headings {
        doc.notes.push(format!("Páginas según {} en este equipo ({total} páginas): cada sección lleva la página de su título.", laid.engine));
    } else {
        doc.notes.push(format!("Páginas según {} en este equipo ({total} páginas); {found} de {headings} títulos ubicados, los demás sin número de página.", laid.engine));
    }
}

/// A bookmark or a line of text that is this title, with or without its numbering.
fn same_title(candidate: &str, title: &str) -> bool {
    candidate == title || candidate.ends_with(&format!(" {title}"))
}

/// A title compared without numbering, case or spacing ("1.2 Alcance" = "alcance").
fn normal(title: &str) -> String {
    let lower = title.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let skip = words.iter().take_while(|w| w.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ')' | '-'))).count();
    words[skip.min(words.len())..].join(" ")
}

/// LibreOffice in headless mode, with its own profile so a LibreOffice the
/// person has open is neither used nor disturbed.
fn libreoffice(source: &Path, dir: &Path) -> Result<String, String> {
    let soffice = find_soffice().ok_or("no está instalado")?;
    let profile = dir.join("perfil");
    let profile_url = format!("file:///{}", profile.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
    let out = std::process::Command::new(&soffice)
        .arg(format!("-env:UserInstallation={profile_url}"))
        .args(["--headless", "--norestore", "--nolockcheck", "--convert-to", "pdf", "--outdir"])
        .arg(dir).arg(source)
        .output().map_err(|e| e.to_string())?;
    if !out.status.success() { return Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
    Ok("LibreOffice".into())
}

fn find_soffice() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["soffice.exe", "soffice.com"] } else { &["soffice", "libreoffice"] };
    let on_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .flat_map(|d| names.iter().map(move |n| d.join(n)));
    // Where the official installers put it when it is not on PATH.
    let installed: Vec<PathBuf> = if cfg!(windows) {
        ["ProgramFiles", "ProgramFiles(x86)"].iter().filter_map(|v| std::env::var_os(v)).map(|p| PathBuf::from(p).join("LibreOffice").join("program").join("soffice.exe")).collect()
    } else if cfg!(target_os = "macos") {
        vec![PathBuf::from("/Applications/LibreOffice.app/Contents/MacOS/soffice")]
    } else { Vec::new() };
    on_path.chain(installed).find(|p| p.is_file())
}

/// Microsoft Word through its automation interface, in its own thread and
/// apartment, never visible, macros disabled, the copy opened read-only.
#[cfg(windows)]
mod word {
    use std::path::Path;
    use windows::core::{BSTR, GUID, HSTRING, PCWSTR};
    use windows::Win32::System::Com::{CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS};
    use windows::Win32::System::Variant::VARIANT;

    const DISPID_PROPERTYPUT: i32 = -3;
    const LOCALE_USER_DEFAULT: u32 = 0x0400;
    const WD_EXPORT_FORMAT_PDF: i32 = 17;
    const WD_EXPORT_CREATE_HEADING_BOOKMARKS: i32 = 1;
    const WD_DO_NOT_SAVE_CHANGES: i32 = 0;
    const MSO_AUTOMATION_SECURITY_FORCE_DISABLE: i32 = 3;

    pub fn export(source: &Path, pdf: &Path) -> Result<String, String> {
        let (source, pdf) = (source.to_path_buf(), pdf.to_path_buf());
        std::thread::spawn(move || unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().map_err(|e| e.message())?;
            let result = run(&source, &pdf);
            CoUninitialize();
            result
        }).join().map_err(|_| "la automatización de Word falló".to_string())?
    }

    unsafe fn run(source: &Path, pdf: &Path) -> Result<String, String> {
        let e = |e: windows::core::Error| e.message();
        let clsid = CLSIDFromProgID(&HSTRING::from("Word.Application")).map_err(|_| "no está instalado".to_string())?;
        let app: IDispatch = CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER).map_err(e)?;
        let version = get(&app, "Version").ok().and_then(|v| BSTR::try_from(&v).ok()).map(|v| v.to_string()).unwrap_or_default();
        let _ = put(&app, "Visible", false.into());
        let _ = put(&app, "DisplayAlerts", 0i32.into());
        let _ = put(&app, "AutomationSecurity", MSO_AUTOMATION_SECURITY_FORCE_DISABLE.into());
        let documents = dispatch(get(&app, "Documents")?)?;
        // A wrong password makes a protected document fail instead of asking.
        let doc = dispatch(call(&documents, "Open", vec![
            ("FileName", source.to_string_lossy().as_ref().into()), ("ConfirmConversions", false.into()), ("ReadOnly", true.into()),
            ("AddToRecentFiles", false.into()), ("PasswordDocument", "docpack-sin-clave".into()), ("Visible", false.into()),
        ])?)?;
        let exported = call(&doc, "ExportAsFixedFormat", vec![
            ("OutputFileName", pdf.to_string_lossy().as_ref().into()), ("ExportFormat", WD_EXPORT_FORMAT_PDF.into()),
            ("OpenAfterExport", false.into()), ("IncludeDocProps", false.into()), ("CreateBookmarks", WD_EXPORT_CREATE_HEADING_BOOKMARKS.into()),
        ]);
        let _ = call(&doc, "Close", vec![("SaveChanges", WD_DO_NOT_SAVE_CHANGES.into())]);
        // Word may be the one the person already has open: it is closed only
        // when no document of theirs is left in it.
        let open = get(&app, "Documents").and_then(dispatch).and_then(|d| get(&d, "Count")).ok().and_then(|c| i32::try_from(&c).ok());
        if open == Some(0) { let _ = call(&app, "Quit", vec![("SaveChanges", WD_DO_NOT_SAVE_CHANGES.into())]); }
        exported?;
        Ok(if version.is_empty() { "Microsoft Word".into() } else { format!("Microsoft Word {version}") })
    }

    fn dispatch(v: VARIANT) -> Result<IDispatch, String> { IDispatch::try_from(&v).map_err(|e| e.message()) }

    unsafe fn ids(object: &IDispatch, names: &[&str]) -> Result<Vec<i32>, String> {
        let wide: Vec<Vec<u16>> = names.iter().map(|n| n.encode_utf16().chain(Some(0)).collect()).collect();
        let ptrs: Vec<PCWSTR> = wide.iter().map(|w| PCWSTR(w.as_ptr())).collect();
        let mut out = vec![0i32; names.len()];
        object.GetIDsOfNames(&GUID::zeroed(), ptrs.as_ptr(), names.len() as u32, LOCALE_USER_DEFAULT, out.as_mut_ptr()).map_err(|_| format!("Word no reconoce {}", names.join(".")))?;
        Ok(out)
    }

    unsafe fn invoke(object: &IDispatch, flags: DISPATCH_FLAGS, name: &str, args: Vec<(&str, VARIANT)>, put: bool) -> Result<VARIANT, String> {
        // A property put passes its value as the one DISPID_PROPERTYPUT argument.
        let names: Vec<&str> = if put { vec![name] } else { std::iter::once(name).chain(args.iter().map(|(n, _)| *n)).collect() };
        let ids = ids(object, &names)?;
        let mut values: Vec<VARIANT> = args.into_iter().map(|(_, v)| v).collect();
        let mut named: Vec<i32> = if put { vec![DISPID_PROPERTYPUT] } else { ids[1..].to_vec() };
        let params = DISPPARAMS { rgvarg: values.as_mut_ptr(), rgdispidNamedArgs: named.as_mut_ptr(), cArgs: values.len() as u32, cNamedArgs: named.len() as u32 };
        let mut result = VARIANT::default();
        object.Invoke(ids[0], &GUID::zeroed(), LOCALE_USER_DEFAULT, flags, &params, Some(&mut result), None, None).map_err(|e| format!("{name}: {}", e.message()))?;
        Ok(result)
    }

    unsafe fn get(object: &IDispatch, name: &str) -> Result<VARIANT, String> { invoke(object, DISPATCH_PROPERTYGET, name, Vec::new(), false) }
    unsafe fn put(object: &IDispatch, name: &str, value: VARIANT) -> Result<VARIANT, String> { invoke(object, DISPATCH_PROPERTYPUT, name, vec![("", value)], true) }
    unsafe fn call(object: &IDispatch, name: &str, args: Vec<(&str, VARIANT)>) -> Result<VARIANT, String> { invoke(object, DISPATCH_METHOD, name, args, false) }
}

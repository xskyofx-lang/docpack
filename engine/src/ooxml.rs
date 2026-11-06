//! Office packages (zip of XML parts), read with the checks an untrusted file
//! needs: no zip bombs, no unsafe names, no encrypted parts, no DTD/entities.
use std::io::{Cursor, Read};

pub type Zip = zip::ZipArchive<Cursor<Vec<u8>>>;

const MAX_PART: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
/// A part may not expand more than this many times its compressed size.
const MAX_RATIO: u64 = 200;

pub fn open(bytes: &[u8]) -> Result<Zip, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).map_err(|_| "Paquete Office inválido o dañado".to_string())?;
    if zip.len() > MAX_ENTRIES { return Err("Paquete Office con demasiadas partes".into()) }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let f = zip.by_index_raw(i).map_err(|_| "Paquete Office inválido".to_string())?;
        if f.encrypted() { return Err("Documento cifrado: no se extrajo contenido".into()) }
        if f.enclosed_name().is_none() { return Err("Paquete Office con rutas inseguras".into()) }
        if f.size() > MAX_PART || (f.compressed_size() > 0 && f.size() > f.compressed_size() * MAX_RATIO && f.size() > 1 << 20) {
            return Err("Paquete Office con expansión desproporcionada (posible zip bomb)".into());
        }
        total += f.size();
    }
    if total > MAX_PART * 8 { return Err("Paquete Office demasiado grande al descomprimir".into()) }
    Ok(zip)
}

/// A part as text; `None` when it does not exist.
pub fn part(zip: &mut Zip, name: &str) -> Result<Option<String>, String> {
    let Ok(file) = zip.by_name(name) else { return Ok(None) };
    let mut text = String::new();
    file.take(MAX_PART).read_to_string(&mut text).map_err(|_| format!("{name} no es XML UTF-8 válido"))?;
    let upper = text.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") { return Err("Documento con DTD o entidades XML: no se procesa".into()) }
    Ok(Some(text))
}

pub fn names(zip: &Zip) -> Vec<String> {
    zip.file_names().filter_map(Result::ok).map(|n| n.into_owned()).collect()
}

pub fn attr(e: &quick_xml::events::BytesStart, name: &str) -> Option<String> {
    e.attributes().flatten().find(|a| { let k = a.key.local_name(); let k: &str = k.as_ref(); k == name }).map(|a| a.value.to_string())
}

/// An attribute by its qualified name (`r:id`, `r:embed`).
pub fn qattr(e: &quick_xml::events::BytesStart, suffix: &str) -> Option<String> {
    e.attributes().flatten().find(|a| { let k: &str = a.key.as_ref(); k.ends_with(suffix) }).map(|a| a.value.to_string())
}

pub fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

pub fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'), "lt" => Some('<'), "gt" => Some('>'), "quot" => Some('"'), "apos" => Some('\''),
        _ => name.strip_prefix("#x").and_then(|h| u32::from_str_radix(h, 16).ok()).or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok())).and_then(char::from_u32),
    }
}

/// Parts of the markup that are not document text: `Fallback` repeats the
/// `Choice` beside it, `tabs`/`tabLst` define tab stops (not tabs in the text).
pub struct Skip(usize);
impl Skip {
    pub fn new() -> Self { Skip(0) }
    pub fn skips(&mut self, event: &quick_xml::Result<quick_xml::events::Event>) -> bool {
        use quick_xml::events::Event;
        match event {
            Ok(Event::Start(e)) => {
                if self.0 > 0 { self.0 += 1; return true }
                if matches!(local(e.name().as_ref()), "Fallback" | "tabs" | "tabLst") { self.0 = 1; return true }
                false
            }
            Ok(Event::End(_)) => { if self.0 > 0 { self.0 -= 1; return true } false }
            Ok(Event::Eof) | Err(_) => false,
            _ => self.0 > 0,
        }
    }
}

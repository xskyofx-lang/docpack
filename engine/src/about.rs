//! What a document says of itself (title, version, date) and whether a part
//! of it is a table of contents. Read from the document's own properties and
//! typography; nothing is guessed from what its words mean.
use crate::About;
use quick_xml::events::Event;

/// The properties of an Office document (`docProps/core.xml`).
pub fn office(bytes: &[u8]) -> About {
    let Ok(mut zip) = crate::ooxml::open(bytes) else { return About::default() };
    let Ok(Some(xml)) = crate::ooxml::part(&mut zip, "docProps/core.xml") else { return About::default() };
    let mut reader = quick_xml::Reader::from_str(&xml);
    let (mut about, mut created) = (About::default(), None);
    let mut field = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => field = crate::ooxml::local(e.name().as_ref()).to_string(),
            Ok(Event::Text(t)) => {
                let text: &str = &t;
                let text = text.trim().to_string();
                if text.is_empty() { continue }
                match field.as_str() {
                    "title" => about.title = Some(text),
                    "version" => about.version = Some(text),
                    "modified" => about.date = text.get(..10).map(str::to_string),
                    "created" => created = text.get(..10).map(str::to_string),
                    _ => {}
                }
            }
            Ok(Event::End(_)) => field.clear(),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if about.date.is_none() { about.date = created }
    about
}

/// The Info dictionary of a PDF: Title, and ModDate (else CreationDate) as
/// YYYY-MM-DD.
pub fn pdf(doc: &lopdf::Document) -> About {
    let info = doc.trailer.get(b"Info").ok().and_then(|i| doc.dereference(i).ok()).and_then(|(_, o)| o.as_dict().ok());
    let Some(info) = info else { return About::default() };
    let text = |key: &[u8]| info.get(key).ok().and_then(|o| o.as_str().ok()).map(pdf_string).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let date = |key: &[u8]| text(key).and_then(|d| {
        let digits: String = d.trim_start_matches("D:").chars().take_while(char::is_ascii_digit).collect();
        (digits.len() >= 8).then(|| format!("{}-{}-{}", &digits[..4], &digits[4..6], &digits[6..8]))
    });
    About { title: text(b"Title"), version: None, date: date(b"ModDate").or_else(|| date(b"CreationDate")) }
}

/// A PDF text string: UTF-16 with its byte order mark, else one byte a letter.
pub(crate) fn pdf_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Fills what the properties did not say from the title or the file name: a
/// version ("v1.4", "V1.8") and a date ("2026-08-25").
pub fn complete(about: &mut About, file_name: &str) {
    let sources: Vec<&str> = about.title.iter().map(String::as_str).chain([file_name]).collect();
    if about.version.is_none() { about.version = sources.iter().find_map(|s| version(s)) }
    if about.date.is_none() { about.date = sources.iter().find_map(|s| iso_date(s)) }
}

/// "v1.4" / "V1.8" / "v2": a v or V after a non-letter, then digits and dots.
fn version(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len()).find_map(|i| {
        let starts = matches!(chars[i], 'v' | 'V') && (i == 0 || !chars[i - 1].is_alphabetic()) && chars.get(i + 1).is_some_and(char::is_ascii_digit);
        if !starts { return None }
        let run: String = chars[i + 1..].iter().take_while(|c| c.is_ascii_digit() || **c == '.').collect();
        Some(run.trim_end_matches('.').to_string())
    })
}

/// A date written YYYY-MM-DD in the text.
fn iso_date(text: &str) -> Option<String> {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(9)).find_map(|i| {
        let w = &b[i..i + 10];
        let ok = w.iter().enumerate().all(|(k, c)| if k == 4 || k == 7 { *c == b'-' } else { c.is_ascii_digit() });
        let month = std::str::from_utf8(&w[5..7]).ok()?.parse::<u8>().ok()?;
        (ok && (1..=12).contains(&month) && (i == 0 || !b[i - 1].is_ascii_digit())).then(|| String::from_utf8_lossy(w).into_owned())
    })
}

/// A part whose lines mostly end in a page number after dot leaders or a tab:
/// a table of contents (at least three such lines, and half of its lines:
/// the rest are its title and the page's header and footer).
pub fn is_contents(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().map(str::trim_end).filter(|l| !l.trim().is_empty() && !l.starts_with("<!--") && !l.starts_with('[')).collect();
    let leader = |l: &&&str| {
        let head = l.trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, 'i' | 'v' | 'x'));
        head.len() < l.len() && (head.ends_with('\t') || head.trim_end().replace('…', "...").ends_with("...."))
    };
    let count = lines.iter().filter(leader).count();
    count >= 3 && count * 2 >= lines.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions_and_dates_from_names() {
        assert_eq!(version("DT_01_Pagos_Servicio_v1.4.docx").as_deref(), Some("1.4"));
        assert_eq!(version("Especificacion Servicio_V1.8.doc.docx").as_deref(), Some("1.8"));
        assert_eq!(version("Layout XML_desc V1.9_act11.xlsx").as_deref(), Some("1.9"));
        assert_eq!(version("servicio.pdf"), None);
        assert_eq!(iso_date("m_SMS_Guide_en-us-2026-08-25.pdf").as_deref(), Some("2026-08-25"));
        assert_eq!(iso_date("acta 2026-13-01.pdf"), None);
    }
    #[test]
    fn a_table_of_contents_is_told_by_its_leaders() {
        assert!(is_contents("1 Alcance ........ 3\n2 Datos ........ 5\n2.1 Campos ........ 7\n"));
        assert!(is_contents("Alcance\t3\nDatos\t5\nCampos\t7\nAnexo\tiv\n"));
        assert!(!is_contents("El total es 3\nLa suma da 5\nQuedan 7\n"));
    }
}

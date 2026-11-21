//! Minimal MCP stdio server (`docpack mcp`) with zero external dependencies.
//! Speaks JSON-RPC 2.0 line-delimited messages over stdin/stdout so any MCP
//! client (Antigravity, Codex, Claude Code, DeepSeek/Kimi clients) can call
//! DocPack tools directly using the same local binary.

use crate::pack_of;
use docpack::{build, library, Options};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// The project folder named in the call, or the folder the client started the
/// server in. Made absolute so packs and replies use one path style.
fn project_dir(args: &Value) -> PathBuf {
    let given = args.get("project").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(PathBuf::from);
    let dir = given.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    std::path::absolute(&dir).unwrap_or(dir)
}

/// A document path from the call. A relative one belongs to the project, not
/// to whatever folder the client happened to start the server in.
fn document(args: &Value, project: &Path) -> Result<PathBuf, String> {
    let given = args.get("file").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).ok_or("Falta file")?;
    let path = Path::new(given);
    let path = if path.is_absolute() { path.to_path_buf() } else { project.join(path) };
    Ok(std::path::absolute(&path).unwrap_or(path))
}

/// What the server tells a client when it connects (MCP `instructions`):
/// the rule and the way through a pack, for an agent that has the tools but
/// not the DocPack skill.
const INSTRUCTIONS: &str = "DocPack turns PDF, Word, Excel and PowerPoint documents into a local pack (<project>/docpack/<file>/) read by parts, so a document costs a fraction of its tokens. \
Never open the original document (no file read, no script that opens it): read its pack. \
1) docpack_read with only file returns the pack's index: sections with pages, token estimate, tables and parts; the pack's notes (manifest) say what its extraction could not do. \
2) Read only the sections the question needs (docpack_read section), a long one by part (part), and only the tables they name (docpack_read table, rows for a large one); tables bring their colors and formulas. \
3) To find something without knowing where it is, docpack_find searches every pack: words, a color (rosa/pink, rojo/red, gris/gray… or RRGGBB) or only tables or text; each hit gives section or table and row, part, line and page. \
4) docpack_text gives given pages; docpack_diff compares two versions. \
Before auditing or reviewing several documents (requirements, absences, spreadsheets, later documents that change earlier ones), call docpack_guide once: it says how to search every way a document may word a thing (find takes several apart with |), why an empty search is not proof (find says what it could not see) and how to cite. \
Cite section, table and page; when a page carries 'impresa N', the document numbers its pages apart: quote the printed number. An agent that can read files may open the pack's files instead (index.json, secciones/*.md, tablas/Tnn.csv, Tnn.colores.json, Tnn.formulas.json).";

fn tool_definitions() -> Value {
    let project = json!({ "type": "string", "description": "Project folder; defaults to the folder the server was started in" });
    let file = json!({ "type": "string", "description": "The document (absolute path, or relative to the project)" });
    json!([
        {
            "name": "docpack_build",
            "description": "Convert a PDF, DOCX, XLSX/XLS/ODS or PPTX document into its local DocPack (<project>/docpack/<file>/): sections in Markdown with page markers, tables as CSV with their colors and formulas, figures, page images, OCR of scanned pages. Built once per document version (by SHA-256); the other tools build it when needed. Returns the pack and index paths, sections, tables and the token estimate. Next: docpack_read with the same file to see the index.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file,
                    "project": project,
                    "pages": { "type": "boolean", "description": "Render the image of every page (default true); false keeps only the pages that need checking" },
                    "force": { "type": "boolean", "description": "Rebuild even if the pack matches the document (default false)" }
                },
                "required": ["file"]
            }
        },
        {
            "name": "docpack_read",
            "description": "Read a document's pack by parts instead of the document. With only file: the index (sections with id, pages, tokens, tables and parts). With section (id such as S05, or its title): that section; a long one returns its parts, then ask for one with part. With table (id such as T03, or its title): its CSV rows numbered from 1 with the header, plus the formulas, colors and review findings (cells with an error, numbers typed among formulas) of the rows shown, each with its cell for a spreadsheet; a large data table needs rows. Every piece starts with a line naming its id, title and pages (and the printed page when the document numbers its pages apart).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "The document (absolute path, or relative to the project), or the name of its pack" },
                    "section": { "type": "string", "description": "Section id (S05) or title" },
                    "part": { "type": "integer", "description": "Part of a long section, from 1" },
                    "table": { "type": "string", "description": "Table id (T03) or title" },
                    "rows": { "type": "string", "description": "Table rows as find names them, such as '1-50' or '4,9'; the header row is always included" },
                    "project": project
                },
                "required": ["file"]
            }
        },
        {
            "name": "docpack_text",
            "description": "The text of given pages (such as '4-5' or '1,3,7-9') from a document's pack, each page under its marker '<!-- página N -->' (with '· impresa M' when the page prints another number). Pages are the PDF's own, counted from 1.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": file,
                    "pages": { "type": "string", "description": "Pages of the PDF, such as '4-5' or '1,3'" },
                    "project": project
                },
                "required": ["file", "pages"]
            }
        },
        {
            "name": "docpack_find",
            "description": "Search every DocPack of the project at once, by words (not by meaning): give every way a document may word the thing apart with |, such as 'multa | sanción | penalidad'; each hit says which one it matched (matched). Each hit names its pack, section (or table, row, column, and the sheet and cell such as D14 for a spreadsheet), part, line and page (and printed_page when the document numbers its pages apart); then read it with docpack_read. Hits are ranked: the words as written first, then a table's title, a heading, body text, a table row; a table of contents last. With no hits, not_found says what the search could not see: pages whose text may be incomplete (OCR, no text, damaged) and documents without a pack; an empty search is not proof of absence. Narrow with pack, in (tablas or texto) or color: only what the document marks in that color, a name in Spanish or English (rosa/pink, rojo/red, gris/gray…) or RRGGBB; with color the query may be left out.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words to search for; several ways of saying it apart with | (optional when color is given)" },
                    "project": project,
                    "pack": { "type": "string", "description": "Only this pack (its name: the document's file name)" },
                    "in": { "type": "string", "enum": ["tablas", "texto"], "description": "Only tables or only text" },
                    "color": { "type": "string", "description": "Color name in Spanish or English (rosa, pink, gris, gray…) or RRGGBB" },
                    "limit": { "type": "integer", "description": "Max hits (default 50)" }
                }
            }
        },
        {
            "name": "docpack_diff",
            "description": "Compare two versions of a document (or their packs): sections added, removed or changed, with the lines that changed, and tables with different rows.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "old": { "type": "string", "description": "Old document, or its pack name or folder" },
                    "new": { "type": "string", "description": "New document, or its pack name or folder" },
                    "project": project
                },
                "required": ["old", "new"]
            }
        },
        {
            "name": "docpack_guide",
            "description": "How to audit or review many documents with DocPack: survey and budget before reading, search every way a document may word a thing (several apart with |), requirements as checklists, later documents that change earlier ones, absence only with proof, spreadsheets (formulas, errors, typed numbers, hidden sheets), one way to cite. Read it once before such a task.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "docpack_catalog",
            "description": "Write <project>/docpack/index.json, the list of every pack with its document's title, version and date, and return its path.",
            "inputSchema": { "type": "object", "properties": { "project": project } }
        },
        {
            "name": "docpack_doctor",
            "description": "Check the health of the project's DocPacks: packs whose document changed or disappeared, incomplete packs, packs made by an older DocPack, documents without a pack and leftovers of interrupted builds.",
            "inputSchema": { "type": "object", "properties": { "project": project } }
        }
    ])
}

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    let proj = project_dir(args);
    let in_project = args.get("project").and_then(Value::as_str).is_some_and(|s| !s.trim().is_empty());
    match name {
        "docpack_build" => {
            let file = document(args, &proj)?;
            let opts = Options {
                pages: args.get("pages").and_then(Value::as_bool).unwrap_or(true),
                force: args.get("force").and_then(Value::as_bool).unwrap_or(false),
                in_project,
            };
            let built = build(&file, &proj, &opts)?;
            Ok(serde_json::to_string_pretty(&built).unwrap_or_default())
        }
        "docpack_text" => {
            let file = document(args, &proj)?;
            let pages_str = args.get("pages").and_then(Value::as_str).ok_or("Falta pages")?;
            let pages = library::parse_pages(pages_str)?;
            let built = build(&file, &proj, &Options { pages: false, force: false, in_project })?;
            library::page_text(&built.pack, &pages)
        }
        "docpack_read" => {
            let given = args.get("file").and_then(Value::as_str).filter(|s| !s.trim().is_empty()).ok_or("Falta file")?;
            let path = document(args, &proj)?;
            // A document is packed first (if needed); a pack is read as it is.
            let pack = if path.is_file() {
                build(&path, &proj, &Options { pages: false, force: false, in_project })?.pack
            } else {
                pack_of(&proj, given, &Options { in_project, ..Options::default() })
            };
            let text = |k: &str| args.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string);
            let what = library::Read {
                section: text("section"),
                part: args.get("part").and_then(Value::as_u64).map(|p| p as usize),
                table: text("table"),
                rows: text("rows"),
            };
            library::read(&pack, &what)
        }
        "docpack_find" => {
            let query = args.get("query").and_then(Value::as_str).unwrap_or("");
            let color = args.get("color").and_then(Value::as_str).map(str::to_string);
            if query.trim().is_empty() && color.is_none() {
                return Err("Se requiere query o color".into());
            }
            let opts = library::Find {
                limit: args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize,
                pack: args.get("pack").and_then(Value::as_str).map(str::to_string),
                only: args.get("in").and_then(Value::as_str).map(str::to_string),
                color,
            };
            let res = library::find_with(&proj.join("docpack"), query, &opts);
            Ok(serde_json::to_string_pretty(&res).unwrap_or_default())
        }
        "docpack_diff" => {
            let old = args.get("old").and_then(Value::as_str).ok_or("Falta old")?;
            let new = args.get("new").and_then(Value::as_str).ok_or("Falta new")?;
            let opts = Options { in_project, ..Options::default() };
            // A relative name is a pack or a document of the project.
            let local = |given: &str| -> String {
                let path = Path::new(given);
                if path.is_relative() && !path.exists() { proj.join(path).to_string_lossy().into_owned() } else { given.to_string() }
            };
            let diff = library::diff(&pack_of(&proj, &local(old), &opts), &pack_of(&proj, &local(new), &opts))?;
            Ok(serde_json::to_string_pretty(&diff).unwrap_or_default())
        }
        "docpack_guide" => Ok(docpack::AUDIT_GUIDE.to_string()),
        "docpack_catalog" => {
            let path = library::write_catalog(&proj.join("docpack"))?;
            Ok(path.display().to_string())
        }
        "docpack_doctor" => {
            let res = library::doctor(&proj);
            Ok(serde_json::to_string_pretty(&res).unwrap_or_default())
        }
        _ => Err(format!("Herramienta desconocida: {name}")),
    }
}

/// MCP revisions whose tool calls this server answers as they define them.
const PROTOCOLS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// The client's revision when this server speaks it; otherwise the newest one
/// it does, and the client decides whether to go on.
fn protocol_version(msg: &Value) -> &'static str {
    let asked = msg.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or("");
    PROTOCOLS.iter().copied().find(|v| *v == asked).unwrap_or(PROTOCOLS[0])
}

pub fn handle_message(msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    match method {
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": protocol_version(msg),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "docpack", "version": docpack::VERSION },
                "instructions": INSTRUCTIONS
            }
        })),
        "tools/list" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": tool_definitions() }
        })),
        "tools/call" => {
            let params = msg.get("params").cloned().unwrap_or(json!({}));
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let result = match call_tool(name, &args) {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
                Err(err) => json!({ "content": [{ "type": "text", "text": err }], "isError": true }),
            };
            Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
        }
        "ping" => Some(json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
        _ => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("Método no soportado: {method}") }
        })),
    }
}

/// The reply to one line of stdin, if it gets one.
fn reply_to_line(line: &str) -> Option<Value> {
    // A client that writes UTF-8 with a byte order mark (PowerShell 5.1 piping
    // to a program does) puts it before the first message.
    let trimmed = line.trim().trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() { return None }
    // JSON-RPC: a line that is not JSON gets a parse error with a null id.
    match serde_json::from_str::<Value>(trimmed) {
        Ok(msg) => handle_message(&msg),
        Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("JSON no válido: {e}") } })),
    }
}

pub fn serve() -> i32 {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if let Some(reply) = reply_to_line(&line) {
            if writeln!(out, "{reply}").is_err() || out.flush().is_err() { break; }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_order_mark_or_a_bad_line_still_gets_an_answer() {
        let listed = reply_to_line("\u{feff}{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\r").unwrap();
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 8);
        let bad = reply_to_line("{bad json").unwrap();
        assert_eq!(bad["error"]["code"], -32700);
        assert!(bad["id"].is_null());
        assert!(reply_to_line("   ").is_none());
    }

    fn call(id: i64, name: &str, arguments: Value) -> Value {
        let reply = handle_message(&json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})).unwrap();
        reply["result"].clone()
    }

    #[test]
    fn lists_all_mcp_tools_and_initializes() {
        let init = handle_message(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}})).unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "docpack");
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18", "a revision the server speaks is echoed");
        let unknown = handle_message(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01"}})).unwrap();
        assert_eq!(unknown["result"]["protocolVersion"], PROTOCOLS[0]);
        let list = handle_message(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 8);
        // A client without the DocPack skill still learns the rule and the
        // way through a pack when it connects.
        let told = init["result"]["instructions"].as_str().unwrap();
        assert!(told.contains("Never open the original") && told.contains("docpack_read") && told.contains("impresa") && told.contains("docpack_guide"), "{told}");
        let guide = call(9, "docpack_guide", json!({}));
        assert!(guide["content"][0]["text"].as_str().unwrap().contains("Absence needs proof"), "{guide}");
        assert!(handle_message(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none(), "notifications get no reply");
    }

    #[test]
    fn a_relative_document_belongs_to_the_project_not_to_the_server_folder() {
        let project = tempfile::tempdir().unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tablas.pdf");
        std::fs::copy(&fixture, project.path().join("tablas.pdf")).unwrap();
        let root = project.path().to_string_lossy().into_owned();

        let built = call(3, "docpack_build", json!({"file":"tablas.pdf","project":root,"pages":false}));
        assert_eq!(built["isError"], false, "{built}");
        assert!(project.path().join("docpack/tablas.pdf/index.json").is_file());

        let text = call(4, "docpack_text", json!({"file":"tablas.pdf","pages":"1","project":root}));
        assert_eq!(text["isError"], false, "{text}");
        assert!(text["content"][0]["text"].as_str().unwrap().contains("página 1"));

        // Without a file reader: the index, then a table by id, by document
        // or by pack name.
        let index = call(5, "docpack_read", json!({"file":"tablas.pdf","project":root}));
        assert_eq!(index["isError"], false, "{index}");
        assert!(index["content"][0]["text"].as_str().unwrap().contains("\"sections\""));
        let table = call(6, "docpack_read", json!({"file":"tablas.pdf","table":"T02","project":root}));
        assert!(table["content"][0]["text"].as_str().unwrap().starts_with("<!-- T02 · "), "{table}");
        std::fs::remove_file(project.path().join("tablas.pdf")).unwrap();
        let by_pack = call(7, "docpack_read", json!({"file":"tablas.pdf","section":"S01","project":root}));
        assert!(by_pack["content"][0]["text"].as_str().unwrap().starts_with("<!-- S01 · "), "{by_pack}");

        let found = call(5, "docpack_find", json!({"query":"Campos","project":root}));
        assert!(found["content"][0]["text"].as_str().unwrap().contains("tablas.pdf"));

        let catalog = call(6, "docpack_catalog", json!({"project":root}));
        assert_eq!(catalog["isError"], false, "{catalog}");

        let diff = call(7, "docpack_diff", json!({"old":"tablas.pdf","new":"tablas.pdf","project":root}));
        assert_eq!(diff["isError"], false, "{diff}");
    }

    #[test]
    fn a_project_without_packs_says_so_instead_of_an_os_error() {
        let project = tempfile::tempdir().unwrap();
        let catalog = call(8, "docpack_catalog", json!({"project":project.path().to_string_lossy()}));
        assert_eq!(catalog["isError"], true);
        assert!(catalog["content"][0]["text"].as_str().unwrap().contains("Todavía no hay packs"));
        assert!(!project.path().join("docpack").exists(), "asking for the catalog writes nothing");
    }
}

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

fn tool_definitions() -> Value {
    json!([
        {
            "name": "docpack_build",
            "description": "Convert a PDF, DOCX, XLSX/XLS/ODS or PPTX document into a local structured DocPack (<project>/docpack/<file>/). Returns index path, section/table counts and token estimate.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the document (absolute, or relative to project)" },
                    "project": { "type": "string", "description": "Optional project folder; defaults to the folder the server was started in" },
                    "pages": { "type": "boolean", "description": "Render page images (default true)" },
                    "force": { "type": "boolean", "description": "Rebuild even if SHA-256 matches (default false)" }
                },
                "required": ["file"]
            }
        },
        {
            "name": "docpack_text",
            "description": "Read extracted text of specific pages (e.g. '4-5' or '1,3,7-9') from a document's DocPack without loading the whole file.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file": { "type": "string", "description": "Path to the document (absolute, or relative to project)" },
                    "pages": { "type": "string", "description": "Page range such as '4-5' or '1,3'" },
                    "project": { "type": "string", "description": "Optional project folder" }
                },
                "required": ["file", "pages"]
            }
        },
        {
            "name": "docpack_find",
            "description": "Search across all DocPacks in a project (or filter by pack, tables/text, or color such as 'rosa', 'rojo', 'FF69B4').",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Text to search for (optional when color is provided)" },
                    "project": { "type": "string", "description": "Optional project folder" },
                    "pack": { "type": "string", "description": "Optional pack name filter" },
                    "in": { "type": "string", "enum": ["tablas", "texto"], "description": "Restrict search to tables or prose" },
                    "color": { "type": "string", "description": "Filter by color name or RRGGBB hex" },
                    "limit": { "type": "integer", "description": "Max hits (default 50)" }
                }
            }
        },
        {
            "name": "docpack_diff",
            "description": "Compare two versions of a document or pack and list added, removed or changed sections and tables.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "old": { "type": "string", "description": "Old document or pack path/name" },
                    "new": { "type": "string", "description": "New document or pack path/name" },
                    "project": { "type": "string", "description": "Optional project folder" }
                },
                "required": ["old", "new"]
            }
        },
        {
            "name": "docpack_catalog",
            "description": "Write and return the path to <project>/docpack/index.json listing all built DocPacks.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Optional project folder" }
                }
            }
        },
        {
            "name": "docpack_doctor",
            "description": "Check the health of the project's DocPacks: packs whose document changed or disappeared, incomplete packs, packs made by an older DocPack, documents without a pack and leftovers of interrupted builds.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Optional project folder" }
                }
            }
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
                "serverInfo": { "name": "docpack", "version": docpack::VERSION }
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
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 6);
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
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 6);
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

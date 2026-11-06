//! Claude Code adapter: the PreToolUse hook protocol. Everything that knows how
//! Claude Code talks lives here; the engine (`lib.rs`) knows nothing of it, so
//! another agent gets its own adapter next to this one.
use docpack::{build, supported, Options};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

/// The project a hook works in: the one Claude Code says, else the cwd it sent.
fn project_dir(input: &Value) -> PathBuf {
    std::env::var_os("CLAUDE_PROJECT_DIR").map(PathBuf::from)
        .or_else(|| input["cwd"].as_str().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Before Claude reads a document, its pack is built (or found current) and
/// Claude is sent to the pack's index instead. Reading named pages of a PDF
/// (`pages`) is the explicit way to look at the original and is let through.
/// Anything unexpected lets the read go on as usual: the hook never blocks
/// work because of its own failure.
pub fn hook() -> i32 {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() { return 0 }
    let Ok(input) = serde_json::from_str::<Value>(&raw) else { return 0 };
    if input["tool_name"] != "Read" { return 0 }
    let Some(file) = input["tool_input"]["file_path"].as_str().map(PathBuf::from) else { return 0 };
    if !supported(&file) || !file.is_file() { return 0 }
    let explicit_pages = input["tool_input"]["pages"].as_str().is_some_and(|p| !p.trim().is_empty());
    if explicit_pages { return 0 }
    let project = project_dir(&input);
    let built = match build(&file, &project, &Options { pages: true, force: false }) {
        Ok(b) => b,
        Err(e) => { eprintln!("docpack: no se pudo crear el pack de {}: {e}", file.display()); return 0 }
    };
    let reason = format!(
        "DocPack: este documento está convertido en {pack} ({sections} secciones, {tables} tablas, {images} imágenes de página, ~{tokens} tokens en total). \
Lee primero {index} y después solo las secciones o tablas que necesites (rutas relativas a la carpeta del pack). \
Las imágenes de paginas/ solo si hace falta ver una figura. Lee el documento original únicamente si la persona lo pidió de forma explícita, \
con Read y el parámetro pages.",
        pack = display(&built.pack), index = display(&built.index), sections = built.sections, tables = built.tables, images = built.page_images, tokens = built.tokens_estimate
    );
    let out = json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason}});
    println!("{out}");
    0
}

fn display(p: &Path) -> String {
    let s = p.to_string_lossy().into_owned();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

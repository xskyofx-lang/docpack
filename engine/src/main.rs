//! docpack build <archivo> [--project <carpeta>] [--no-pages] [--force]
//! docpack text <archivo> --pages 4-5 [--project <carpeta>]
//! docpack read <archivo o pack> [--section S05 [--part 2]] [--table T03 [--rows 1-50]] [--project <carpeta>]
//! docpack coverage <requisitos.txt|json> [--limit 3] [--pack <nombre>] [--in tablas|texto] [--project <carpeta>]
//! docpack guide              (how to audit many documents with DocPack)
//! docpack find "<texto>" [--project <carpeta>] [--limit N]
//! docpack diff <pack o documento> <pack o documento> [--project <carpeta>]
//! docpack doctor [--project <carpeta>]
//! docpack catalog [--project <carpeta>]
//! docpack mcp                 (MCP stdio server; JSON-RPC 2.0)
//! docpack hook claude-code   (PreToolUse hook for Claude Code; JSON on stdin)
//! docpack --version
mod claude_code;
mod mcp;

use docpack::{build, library, Options};
use std::path::{Path, PathBuf};

const USAGE: &str = "Uso:
  docpack build <archivo> [--project <carpeta>] [--no-pages] [--force]
  docpack text <archivo> --pages 4-5 [--project <carpeta>]
  docpack read <archivo o pack> [--section S05 [--part 2]] [--table T03 [--rows 1-50]] [--project <carpeta>]
  docpack coverage <requisitos.txt o .json> [--limit 3] [--pack <nombre>] [--in tablas|texto] [--project <carpeta>]
  docpack guide
  docpack find \"<texto>\" [--project <carpeta>] [--limit N] [--pack <nombre>] [--in tablas|texto] [--color <nombre o RRGGBB>]
  docpack diff <pack o documento> <pack o documento> [--project <carpeta>]
  docpack doctor [--project <carpeta>]
  docpack catalog [--project <carpeta>]
  docpack mcp
  docpack hook claude-code
  docpack --version";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let code = match args.first().map(String::as_str) {
        Some("build") => cmd_build(rest),
        Some("text") => cmd_text(rest),
        Some("read") => cmd_read(rest),
        Some("coverage") => cmd_coverage(rest),
        Some("guide") => { print!("{}", docpack::AUDIT_GUIDE); 0 }
        Some("find") => cmd_find(rest),
        Some("diff") => cmd_diff(rest),
        Some("doctor") => cmd_doctor(rest),
        Some("catalog") => cmd_catalog(rest),
        Some("mcp") => mcp::serve(),
        Some("hook") => match args.get(1).map(String::as_str) {
            Some("claude-code") => claude_code::hook(),
            _ => { eprintln!("{USAGE}"); 2 }
        },
        Some("--version") | Some("-V") | Some("version") => { println!("docpack {}", docpack::VERSION); 0 }
        _ => { eprintln!("{USAGE}"); 2 }
    };
    std::process::exit(code);
}

/// The arguments of a command: its words, and the value of each `--flag`
/// that takes one (`--project`, `--pages`, `--limit`); bare flags map to "".
struct Args { words: Vec<String>, flags: std::collections::HashMap<String, String> }

fn parse(args: &[String], valued: &[&str], bare: &[&str]) -> Result<Args, String> {
    let (mut words, mut flags) = (Vec::new(), std::collections::HashMap::new());
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if let Some(flag) = a.strip_prefix("--") {
            if valued.contains(&flag) {
                i += 1;
                flags.insert(flag.to_string(), args.get(i).cloned().ok_or(format!("Falta el valor de --{flag}"))?);
            } else if bare.contains(&flag) {
                flags.insert(flag.to_string(), String::new());
            } else {
                return Err(format!("Argumento desconocido: {a}"));
            }
        } else {
            words.push(a.clone());
        }
        i += 1;
    }
    Ok(Args { words, flags })
}

fn project(args: &Args) -> PathBuf {
    args.flags.get("project").map(PathBuf::from).unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn print_json(value: &impl serde::Serialize) -> i32 {
    println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
    0
}

fn fail(e: impl std::fmt::Display) -> i32 { eprintln!("docpack: {e}"); 1 }

macro_rules! parsed {
    ($args:expr, $valued:expr, $bare:expr) => {
        match parse($args, $valued, $bare) { Ok(a) => a, Err(e) => { eprintln!("{e}\n{USAGE}"); return 2 } }
    };
}

fn cmd_build(args: &[String]) -> i32 {
    let args = parsed!(args, &["project"], &["no-pages", "force"]);
    let [file] = args.words.as_slice() else { eprintln!("Falta el archivo\n{USAGE}"); return 2 };
    let options = Options { pages: !args.flags.contains_key("no-pages"), force: args.flags.contains_key("force"), in_project: args.flags.contains_key("project") };
    match build(Path::new(file), &project(&args), &options) { Ok(built) => print_json(&built), Err(e) => fail(e) }
}

/// The text of some pages, from the document's pack (built if needed): the
/// way to look at pages without opening the original.
fn cmd_text(args: &[String]) -> i32 {
    let args = parsed!(args, &["project", "pages"], &[]);
    let [file] = args.words.as_slice() else { eprintln!("Falta el archivo\n{USAGE}"); return 2 };
    let pages = match library::parse_pages(args.flags.get("pages").map(String::as_str).unwrap_or("")) { Ok(p) => p, Err(e) => { eprintln!("{e}"); return 2 } };
    let built = match build(Path::new(file), &project(&args), &Options { pages: false, force: false, in_project: args.flags.contains_key("project") }) { Ok(b) => b, Err(e) => return fail(e) };
    match library::page_text(&built.pack, &pages) { Ok(text) => { print!("{text}"); 0 } Err(e) => fail(e) }
}

/// A piece of a pack: its index, a section (or a part of it) or a table (or
/// some of its rows). A document is packed first when needed.
fn cmd_read(args: &[String]) -> i32 {
    let args = parsed!(args, &["project", "section", "part", "table", "rows"], &[]);
    let [given] = args.words.as_slice() else { eprintln!("Falta el archivo o el pack\n{USAGE}"); return 2 };
    let part = match args.flags.get("part").map(|p| p.parse::<usize>()) {
        None => None,
        Some(Ok(p)) => Some(p),
        Some(Err(_)) => { eprintln!("--part es un número desde 1\n{USAGE}"); return 2 }
    };
    let (project, options) = (project(&args), Options { pages: false, force: false, in_project: args.flags.contains_key("project") });
    let pack = if Path::new(given).is_file() {
        match build(Path::new(given), &project, &options) { Ok(b) => b.pack, Err(e) => return fail(e) }
    } else { pack_of(&project, given, &options) };
    let what = library::Read { section: args.flags.get("section").cloned(), part, table: args.flags.get("table").cloned(), rows: args.flags.get("rows").cloned() };
    match library::read(&pack, &what) { Ok(text) => { print!("{text}"); 0 } Err(e) => fail(e) }
}

/// A coverage matrix: each requirement of a list (and each of its parts)
/// searched in every pack at once, with the best places that say it.
fn cmd_coverage(args: &[String]) -> i32 {
    let args = parsed!(args, &["project", "limit", "pack", "in"], &[]);
    let [file] = args.words.as_slice() else { eprintln!("Falta la lista de requisitos\n{USAGE}"); return 2 };
    let only = args.flags.get("in").cloned();
    if only.as_deref().is_some_and(|o| o != "tablas" && o != "texto") { eprintln!("--in es tablas o texto\n{USAGE}"); return 2 }
    let text = match std::fs::read_to_string(file) { Ok(t) => t, Err(e) => return fail(format!("{file}: {e}")) };
    let requirements = match library::parse_requirements(&text) { Ok(r) => r, Err(e) => return fail(e) };
    let opts = library::Find { limit: args.flags.get("limit").and_then(|l| l.parse().ok()).unwrap_or(3), pack: args.flags.get("pack").cloned(), only, color: None };
    print_json(&library::coverage(&project(&args).join("docpack"), &requirements, &opts))
}

fn cmd_find(args: &[String]) -> i32 {
    let args = parsed!(args, &["project", "limit", "pack", "in", "color"], &[]);
    let color = args.flags.get("color").cloned();
    if args.words.is_empty() && color.is_none() { eprintln!("Falta el texto a buscar (o --color)\n{USAGE}"); return 2 }
    let only = args.flags.get("in").cloned();
    if only.as_deref().is_some_and(|o| o != "tablas" && o != "texto") { eprintln!("--in es tablas o texto\n{USAGE}"); return 2 }
    let opts = library::Find { limit: args.flags.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50), pack: args.flags.get("pack").cloned(), only, color };
    print_json(&library::find_with(&project(&args).join("docpack"), &args.words.join(" "), &opts))
}

/// A pack folder as given, the pack of that name in the project, or the pack
/// of a document of the project.
fn pack_of(project: &Path, given: &str, options: &Options) -> PathBuf {
    let path = PathBuf::from(given);
    if path.join("manifest.json").is_file() { return path }
    let named = project.join("docpack").join(given);
    if named.join("manifest.json").is_file() { return named }
    docpack::pack_dir_with(project, &path, options)
}

fn cmd_diff(args: &[String]) -> i32 {
    let args = parsed!(args, &["project"], &[]);
    let [a, b] = args.words.as_slice() else { eprintln!("Faltan las dos versiones\n{USAGE}"); return 2 };
    let (project, options) = (project(&args), Options { in_project: args.flags.contains_key("project"), ..Options::default() });
    match library::diff(&pack_of(&project, a, &options), &pack_of(&project, b, &options)) { Ok(d) => print_json(&d), Err(e) => fail(e) }
}

fn cmd_doctor(args: &[String]) -> i32 {
    let args = parsed!(args, &["project"], &[]);
    print_json(&library::doctor(&project(&args)))
}

fn cmd_catalog(args: &[String]) -> i32 {
    let args = parsed!(args, &["project"], &[]);
    match library::write_catalog(&project(&args).join("docpack")) { Ok(path) => { println!("{}", path.display()); 0 } Err(e) => fail(e) }
}

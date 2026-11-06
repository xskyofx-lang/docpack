//! docpack build <archivo> [--project <carpeta>] [--no-pages] [--force]
//! docpack hook claude-code   (PreToolUse hook for Claude Code; JSON on stdin)
//! docpack --version
mod claude_code;

use docpack::{build, Options};
use std::path::PathBuf;

const USAGE: &str = "Uso:\n  docpack build <archivo> [--project <carpeta>] [--no-pages] [--force]\n  docpack hook claude-code\n  docpack --version";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("build") => cmd_build(&args[1..]),
        Some("hook") => match args.get(1).map(String::as_str) {
            Some("claude-code") => claude_code::hook(),
            _ => { eprintln!("{USAGE}"); 2 }
        },
        Some("--version") | Some("-V") | Some("version") => { println!("docpack {}", docpack::VERSION); 0 }
        _ => { eprintln!("{USAGE}"); 2 }
    };
    std::process::exit(code);
}

fn cmd_build(args: &[String]) -> i32 {
    let mut file: Option<PathBuf> = None;
    let mut project: Option<PathBuf> = None;
    let mut options = Options { pages: true, force: false };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--project" => { i += 1; project = args.get(i).map(PathBuf::from) }
            "--no-pages" => options.pages = false,
            "--force" => options.force = true,
            other if file.is_none() => file = Some(PathBuf::from(other)),
            other => { eprintln!("Argumento desconocido: {other}"); return 2 }
        }
        i += 1;
    }
    let Some(file) = file else { eprintln!("Falta el archivo"); return 2 };
    let project = project.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    match build(&file, &project, &options) {
        Ok(built) => { println!("{}", serde_json::to_string_pretty(&built).unwrap_or_default()); 0 }
        Err(e) => { eprintln!("docpack: {e}"); 1 }
    }
}

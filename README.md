# DocPack

**English** · [Español](README.es.md)

**Your AI reads page 40 without reading the other 2,400.**

When you hand an AI agent a big PDF, Word or Excel file, it reads the whole
thing, even if you only need one table. That costs tokens, fills the context,
and long documents don't even fit in one read.

DocPack converts the document **on your machine** into a folder of parts: a
small index, one Markdown file per section, one CSV per table, the colors the
document marks things with, its formulas and its page images. The agent opens
the index, sees what is there and how many tokens each part weighs, and reads
only what answers the question.

- **Local and free.** One ~3 MB binary. No Python, no Node, no internet, no
  API. Converting costs 0 tokens.
- **Works with your agent.** Claude Code (plugin), OpenAI Codex, Kimi Code,
  Google Antigravity and DeepSeek clients (MCP server + skill), Muse, and
  [Sky Agent](https://ia.skyoflies.com), where it is built in.
- **Keeps what everyday documents carry**: rows in gray, text in red, "test
  only the pink ones", formulas, padded Excel exports, titles above tables,
  borderless PDF tables, tables split across pages, scanned pages.

<p align="center"><img src="docs/img/savings.svg" alt="Tokens to answer about 44 real documents: 27.8 million reading them whole ($111.08 with Claude Opus 5.5), 166,725 with DocPack on the first question (99.4% less) and 23,368 on the next ones (99.9% less)" width="720"></p>

## Quickstart

**Install it for your agent** (Claude Code has its own plugin, see
[Install](#install)):

```powershell
# Windows (PowerShell)
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent codex
```

```bash
# macOS and Linux
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent codex
```

Use `kimi`, `antigravity` or `deepseek` instead of `codex`. The installer puts
the `docpack` binary on your machine, the skill where your agent looks for
skills, and registers the `docpack mcp` server. Then just ask your agent about
your document.

**Or try it yourself** on the documents in [`examples/`](examples/):

```bash
docpack build clinic-inventory.xlsx      # index, sections, CSV tables, colors, formulas
docpack find --color pink                # only what the documents mark in pink
docpack diff vaccination-plan-v1.0.docx vaccination-plan-v1.1.docx
```

## Why it is not another Markdown converter

Real documents are not clean text. The examples in [`examples/`](examples/)
are the kind people send every day: an Excel inventory and two versions of a
Word plan. Below is a summary of DocPack's real output on them (the output
itself is JSON).

**"Test only the pink ones."** The plan highlights in pink what goes into the
first delivery. `docpack find --color pink` returns exactly those rows, with
their table, section and page:

```text
2. Schedule · T01 row 2   Dog | Parvovirus | 6 weeks | 1 ml
2. Schedule · T01 row 4   Cat | Triple feline | 8 weeks | 0.5 ml
```

Each color in the document has its name, its hex value and where it is used
(text, fill, highlight). `--color` takes the name in English or Spanish
(`pink` or `rosa`, `gray`/`grey` or `gris`, `red`, `yellow`…) or an exact hex
value such as `FF00FF`. The pack itself writes color names in Spanish
(`rojo`, `rosa`, `gris`, `amarillo`…).

**What changed between versions.** `docpack diff` compares sections and table
rows, not bytes:

```text
Table "2. Schedule"   − Dog | Rabies | 12 weeks | 1 ml
                      + Dog | Rabies | 12 weeks | 1.5 ml
                      + Rabbit | Myxomatosis | 10 weeks | 0.5 ml
Section "1. Scope"    + … From version 1.1 it also covers rabbits. …
Section "3. Records"  + … Rabbits get a card of their own.
```

**Excel as people really use it.** In the inventory:

| In the workbook | In the pack |
|---|---|
| Cells padded with spaces (a fixed-width export) | removed, and the manifest says how many. On a real 10-sheet export: 2.18 MB → 231 KB of CSV |
| `=D4*E4`, `=SUMIF(Inventory!A4:A10,"VAC*",…)` | the CSV keeps the value; `Tnn.formulas.json` has each cell's formula |
| A merged title above the table | names the table; the header is the real header row |
| A row in red, a row in gray, a yellow cell, one word in red inside a cell | `Tnn.colores.json` per cell and a color legend in the manifest |

## What it reads

| Format | Sections | Tables | Also |
|---|---|---|---|
| Word (DOCX) | one per heading, from the document's styles | CSV, with each cell's color | page of each section and page images (with Word or LibreOffice), embedded images, list levels, headers and footers, colored and highlighted text |
| PDF | from the PDF's bookmarks, else one per page; finer bookmarks become `###` subheadings | CSV, ruled or borderless, joined across pages, with each cell's color | OCR of scanned pages, page images, table of contents marked, rotated pages straightened, running headers and footers removed |
| Excel (XLSX, XLS, XLSB, ODS) | one per sheet | one CSV per sheet, with its real header under any title rows | each cell's formula, padding removed, cell fills and text colors (XLSX) |
| PowerPoint (PPTX) | one per slide | CSV | slide number |

A long section is split into parts of about 2,000 tokens with their line
numbers, so the agent reads only the stretch it needs. When a document numbers
its tables ("Table 16:"), DocPack checks every one against its CSVs and names
the ones it could not read in the manifest, with their page image.

## Benchmarks

Measured on 44 real working documents: 27 PDF technical guides (2 to 3,220
pages), 7 Word designs and 10 Excel workbooks. "Without DocPack" is reading
the whole document; "with DocPack" is its index plus the part the question
needs.

| 44 documents | Without DocPack | With DocPack | Savings |
|---|---:|---:|---:|
| First question on each document | 27.8 M tokens · $111.08 | 166,725 · $0.67 | **99.4%** |
| Each next question | 27.8 M · $111.08 | 23,368 · $0.09 | **99.9%** |
| Median per document (1st · next) | | | **94.9% · 99.6%** |
| Documents too big for one conversation (over 1 M tokens) | 6 | 0 | |

The savings are per question: a two-page document saves little, because its
index weighs half of it. The 44 documents are private working documents and
cannot be published; the [`examples/`](examples/) and the tests in
`engine/tests/` are public and reproduce every behavior described here. All
the numbers, speed and fidelity checks (6,393 of 6,393 table cells, 98% of the
tables of a 2,409-page borderless guide) are in
[BENCHMARKS.md](BENCHMARKS.md).

## Install

DocPack runs on your machine and reads your documents the same way in every
agent: the same binary, the same pack and the same reading rules.

| | Agent | How it connects | Install |
|:-:|---|---|---|
| <img src="docs/assets/agentes/claude.svg" width="32" height="32" alt="Claude"> | **Claude Code** | Plugin (hook + skill) | [Claude Code](#claude-code) |
| <img src="docs/assets/agentes/muse.svg" width="32" height="32" alt="Muse"> | **Muse** (Meta) | Local install + skill | [Muse](#muse) |
| <img src="docs/assets/agentes/codex.svg" width="32" height="32" alt="OpenAI Codex"> | **OpenAI Codex** | MCP server + skill | [Codex](#openai-codex) |
| <img src="docs/assets/agentes/kimi.svg" width="32" height="32" alt="Kimi"> | **Kimi Code** | MCP server + skill | [Kimi](#kimi-code) |
| <img src="docs/assets/agentes/antigravity.svg" width="32" height="32" alt="Google Antigravity"> | **Google Antigravity** | MCP server + skill | [Antigravity](#google-antigravity) |
| <img src="docs/assets/agentes/deepseek.svg" width="32" height="32" alt="DeepSeek"> | **DeepSeek** (DeepSeek-TUI, OpenCode, Cline…) | MCP server + skill | [DeepSeek](#deepseek) |

### Claude Code

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/claude-oscuro.png">
  <img src="docs/img/claude.png" alt="Claude" height="48">
</picture>

Inside Claude Code:

```
/plugin marketplace add xskyofx-lang/docpack
/plugin install docpack@docpack
/reload-plugins
```

Or from the terminal:

```bash
claude plugin marketplace add xskyofx-lang/docpack
```

```bash
claude plugin install docpack@docpack
```

That is all. No Python, no Rust, nothing else to install: the plugin carries
the DocPack binary already built.

**Systems:** Windows (x64), Linux (x64 and ARM64) and macOS (Apple Silicon and
Intel). The plugin carries the binary for each one and picks the right one.
When Claude tries to read a document, the plugin's hook builds (or reuses) its
pack and points Claude to its index.

Some features use what your system already has. If something is missing the
pack is still built and `manifest.json` says what was not done and why:

| Feature | Windows | macOS | Linux |
|---|---|---|---|
| Page images | built into Windows | built into macOS | `pdftoppm` (`poppler-utils` package) |
| OCR of scanned pages | Windows OCR, in your profile's languages | macOS Vision, in your languages | `tesseract` with its languages (for example `tesseract-ocr-spa`) |
| Page numbers in Word | Microsoft Word or LibreOffice | LibreOffice | LibreOffice |

### Muse

<p><img src="docs/img/muse.jpg" alt="Muse" height="160"></p>

Muse does not install plugins, but its agent has its own Linux machine and can
install programs. Send it this message:

```
Install DocPack on your machine: download and run
https://raw.githubusercontent.com/xskyofx-lang/docpack/main/plugins/muse/install.sh
then save as a skill the file
https://raw.githubusercontent.com/xskyofx-lang/docpack/main/plugins/muse/SKILL.md
so from now on you read my PDF, Word, Excel and PowerPoint documents through
their pack, not the original file.
```

- `install.sh` detects the platform (Linux x64 or ARM64, macOS), downloads the
  same binary the Claude Code plugin carries to `~/.local/bin/docpack` and
  checks it with `docpack --version`. No Rust, Python or building needed.
- `SKILL.md` is the reading-by-parts rule, the same one Claude follows.
- **Updates:** once a week Muse runs `check-update.sh`, which compares its
  version with the one published in this repository. If there is a new one it
  tells you which and what it improves (from the CHANGELOG) and asks whether to
  install it; it never updates without your permission.

Tested on Muse's machine on November 14, 2026 with a real Word document.
The adapter's details are in [plugins/muse/README.md](plugins/muse/README.md).

### OpenAI Codex

<img src="docs/assets/agentes/codex.svg" width="48" height="48" alt="OpenAI Codex">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent codex
```

**macOS and Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent codex
```

Registers the server with `codex mcp add` (saved in `~/.codex/config.toml`) and puts the skill in `~/.agents/skills/docpack/`. Before editing a configuration it keeps a copy `*.antes-de-docpack`.
Restart the agent when it finishes. Details in [plugins/codex/README.md](plugins/codex/README.md).

### Kimi Code

<img src="docs/assets/agentes/kimi.svg" width="48" height="48" alt="Kimi Code">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent kimi
```

**macOS and Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent kimi
```

Registers the server in `~/.kimi-code/mcp.json` and puts the skill in `~/.agents/skills/docpack/`. In Kimi, `/mcp` shows whether it is connected. Before editing a configuration it keeps a copy `*.antes-de-docpack`.
Restart the agent when it finishes. Details in [plugins/kimi/README.md](plugins/kimi/README.md).

### Google Antigravity

<img src="docs/assets/agentes/antigravity.svg" width="48" height="48" alt="Google Antigravity">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent antigravity
```

**macOS and Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent antigravity
```

Registers the server in `~/.gemini/config/mcp_config.json` (shared by the app, the IDE and the CLI) and puts the skill in `~/.gemini/config/skills/docpack/`. Before editing a configuration it keeps a copy `*.antes-de-docpack`.
Restart the agent when it finishes. Details in [plugins/antigravity/README.md](plugins/antigravity/README.md).

### DeepSeek

<img src="docs/assets/agentes/deepseek.svg" width="48" height="48" alt="DeepSeek">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent deepseek
```

**macOS and Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent deepseek
```

DeepSeek has no single configuration file: the installer puts the skill in `~/.agents/skills/docpack/` and prints the exact MCP entry to paste into your client (DeepSeek-TUI, OpenCode, Cline, Roo Code…). Before editing a configuration it keeps a copy `*.antes-de-docpack`.
Restart the agent when it finishes. Details in [plugins/deepseek/README.md](plugins/deepseek/README.md).

### Only the `docpack` command

To use it from the terminal or connect it to another MCP client
(`docpack mcp` is an MCP server over stdio):

```powershell
irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1 | iex
```

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash
```

It installs the binary in `%LOCALAPPDATA%\Programs\DocPack` (and adds it to
your user PATH; `-NoPath` leaves the PATH alone) or in `~/.local/bin`.

Or download the binary for your system from the
[latest release](https://github.com/xskyofx-lang/docpack/releases/latest)
(with its SHA-256 sums), or build it with Rust:

```bash
cargo install --git https://github.com/xskyofx-lang/docpack docpack --locked
```

## Commands

| Command | What for |
|---|---|
| `docpack build <file>` | Build or reuse the pack (`--force` rebuilds, `--no-pages` skips page images) |
| `docpack text <file> --pages 4-5` | The text of those pages, from the pack |
| `docpack read <file> [--section S05 [--part 2]] [--table T03 [--rows 1-50]]` | The index (with the pack's notes), a section or one part of it, or a table with its formulas and colors: for an agent without a file reader |
| `docpack find "text"` | Search every pack of the project: pack, section or table, page and line. Filters: `--pack`, `--in tablas`, `--in texto` |
| `docpack find "fine \| penalty \| sanction"` | Several ways of saying one thing in one search; each hit says which it matched. With no hits, it says what the search could not see (pages read by OCR or without text, documents without a pack) |
| `docpack coverage requirements.txt` | A coverage matrix in one pass: each requirement (`name = way \| other way`, parts indented below) found or not, with its best places cited and its document's date. Evidence to read, not a verdict |
| `docpack guide` | How to audit many documents with DocPack: for an agent (also the MCP tool `docpack_guide`) |
| `docpack find --color pink` | What the documents mark in that color: name in English or Spanish (`pink`, `rosa`) or hex (`--color FF00FF`) |
| `docpack diff <old> <new>` | What changed between two versions: sections and table rows |
| `docpack doctor` | Half-built or outdated packs, foreign files and documents with no pack |
| `docpack catalog` | Rewrites `docpack/index.json` (it also updates with every pack) |
| `docpack mcp` | MCP server over stdio: `docpack_build`, `docpack_read`, `docpack_text`, `docpack_find`, `docpack_diff`, `docpack_coverage`, `docpack_guide`, `docpack_catalog`, `docpack_doctor` |

All of them take `--project <folder>`. A `.docpackignore` file in the project
(one pattern per line, such as `export_*.xls`) leaves out data files that are
not worth converting.

## What a pack contains

```
your-project/
  plan.docx
  docpack/
    index.json          every pack with its document's title, version and date
    plan.docx/
      manifest.json     source, SHA-256, title/version/date, notes, color legend
      index.json        sections with pages, lines and token estimate: read first
      secciones/        01-scope.md, 02-schedule.md …
      tablas/           T01.csv …, index.json (title, header, pages),
                        Tnn.colores.json (colors), Tnn.formulas.json (formulas)
      figuras/          F01.png … (images embedded in Word)
      paginas/          p001.png … (page images, all or only the ones to check)
```

The original document is never changed. A pack rebuilds itself when the
document changes (its SHA-256 no longer matches) or when DocPack is updated.
Folder and field names, and the notes in the manifest, are in Spanish today;
their meaning is documented in [docs/integrar-otro-agente.md](docs/integrar-otro-agente.md).

## Updates

Each release has its number in `plugin.json`. In Claude Code:

```bash
claude plugin marketplace update docpack
claude plugin update docpack@docpack
```

or turn on auto-update in `/plugin` → **Marketplaces** → docpack. For the other
agents, run the installer again. What changed in each version is in
[CHANGELOG.md](CHANGELOG.md) (in Spanish).

## Repository

```
engine/                 the engine: library and `docpack` command (Rust)
  src/lib.rs            converts and builds the pack, knows nothing about agents
  src/claude_code.rs    the only file that knows Claude Code's hook protocol
  src/mcp.rs            the MCP server (docpack mcp) for the other agents
  vendor/pdf-extract/   pdf-extract 0.12.1 (MIT) with font widths and colors fixed
plugins/                one folder per agent: claude-code, codex, kimi,
                        antigravity, deepseek, muse
install/                installers for Windows and macOS/Linux
examples/               everyday documents and the script that makes them
docs/                   pack format, adding another agent, logos, charts
.github/workflows/      tests on the three systems, binaries, installer tests
```

Release binaries are not built on anyone's machine: GitHub Actions builds them
on Windows, Linux and macOS (workflow **Binarios**), checks each one answers
with the plugin's version, and commits them to `plugins/claude-code/`. The
installers are tested on every system for every agent (workflow
**Instaladores**).

To build from source you need [Rust](https://rustup.rs) 1.80 or newer:

```bash
cargo build --release --manifest-path engine/Cargo.toml
```

## License

MIT. Use it, change it and share it; keep the authorship notice.

The logos in `docs/assets/agentes/` are trademarks of their owners and are
shown only to say which agents DocPack works with. They come from
[Simple Icons](https://simpleicons.org) (CC0) and, for Antigravity, from
Google Antigravity's site.

Made by Juan Antonio Sandoval Puente · [skyoflies.com](https://skyoflies.com)

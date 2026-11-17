---
name: docpack
description: Read PDF, Word (DOCX), Excel (XLSX/XLS/ODS) and PowerPoint (PPTX) documents through their DocPack instead of the original file. Use whenever the person mentions, attaches by path, or asks about a document in one of these formats, or when a Read of such a file was redirected to a pack.
---

# DocPack: read documents by parts

Every supported document has a pack at `<project>/docpack/<file name>/`, built
locally (no tokens are spent converting). The original file is never changed.
`<project>/docpack/index.json` lists every pack with its document's title,
version and date.

## The rule

1. **Never read the original document**: not with Read, not with `cat`, not
   with a Python or shell script that opens the PDF, DOCX or XLSX. Read the pack:
   - `index.json` first: sections (with pages and an estimated token count),
     tables and page images. In a long document each section is one line, «id · pages · tokens ·
     tables · parts · title» (S491's file is `secciones/491-….md`), and
     `section_detail` names `secciones/index.json`, where each section's
     file, `parts`, `outline`, tables and figures are: look up only the
     section you chose. When a document has hundreds of subsections, the
     index lists only its top level with the range of their subsections, and
     `section_list` names `secciones/lista.txt`, every section in one line:
     grep it for a title (or use `docpack find`) instead of reading it whole. A section with `"kind": "toc"` is a table of
     contents: it says where things are, not what they say.
   - Then only the `secciones/*.md` the question needs, and of them only the
     tables (`tablas/Tnn.csv`) and figures they reference. `tablas/index.json`
     gives each table's title, header, page (and `pages` when it goes on across
     pages), so you can pick a table without opening it. A table with a
     `check` field holds another table's caption among its cells: compare it
     with its page image before quoting it.
   - A long section lists `parts` (line ranges with their first line, or the
     first subheading they hold) and, when its lists have groups, an `outline`
     with lines: read just that part with Read `offset`/`limit` instead of the
     whole section. PDF subheadings from the document's bookmarks are Markdown
     headings (`###`) in the section, so a field or subfield can be found with
     a search for its heading.
   - A table marked `"read"` in `tablas/index.json` is data (thousands of
     rows): filter or summarize its CSV with a script; do not read it whole.
   - **Colors.** When the document marks things with color (an item in red
     that does not apply, a gray row not delivered yet, "test only the pink
     ones"), `manifest.json` → `colores` is its legend: each color with its
     name (rojo, rosa, morado, gris…), hex value, use (texto, fondo,
     resaltado), count and examples; `index.json` → `colores` lists them in
     short. A table with colors names `colores_file` in `tablas/index.json`:
     `tablas/Tnn.colores.json` gives, per cell (`fila`, `columna` as in the
     CSV, header row 0), its `fondo` and the pieces of its text in a color of
     their own. The CSV keeps the plain text. In a section, marked text is
     `<span style="color:#RRGGBB">` or `<mark style="background:#RRGGBB">`.
     To answer "which are pink", filter the cells of `Tnn.colores.json` by
     the color the legend names; never guess a color from the words. Text all
     in one color on its line in a PDF (a heading, a caption) is the
     document's style and is not marked.
   - **Formulas.** A spreadsheet table with formulas names `formulas_file`
     in `tablas/index.json`: `tablas/Tnn.formulas.json` lists each calculated
     cell (`fila`, `columna` as in the CSV, header row 0) with its formula,
     such as `=D4*E4` or `=SUMIF(Inventory!A4:A10,"VAC*",Inventory!F4:F10)`.
     The CSV keeps the value the cell shows; read the formula to say how a
     number is calculated.
   - `paginas/pNNN.png` only when you must see a figure or a layout.
2. **To look at given pages, use the pack**:
   ```bash
   docpack text "<document>" --pages 4-5 --project "<project folder>"
   ```
   It prints those pages' text from the pack (building it if needed). Read the
   original only when the person explicitly asks for the original file, and
   then with Read and the `pages` parameter for exactly the pages they named.
3. **To find something across documents**, search every pack at once instead
   of opening them one by one:
   ```bash
   docpack find "vacuna antirrábica" --project "<project folder>"
   ```
   Each hit names its pack, section (or table and row), part, page and line.
   Hits come ranked: the words as written first, then a table's title, a
   heading, body text and a table's row; a table of contents last. A line
   repeated word for word in a pack (a running header) is one hit with
   `count`. Narrow it with `--pack <name>`, `--in tablas` or `--in texto`,
   and `--color rosa` (or `--color FF69B4`) for only what the document marks
   in that color; with `--color` the text may be left out
   (`docpack find --color rosa`) and each hit lists the colored cells.
4. **To compare two versions** of a document:
   ```bash
   docpack diff "<old document or pack>" "<new document or pack>" --project "<project folder>"
   ```
   It lists sections added, removed or changed and tables with different rows.
5. **Cite where things come from**: section title and page (sections carry
   their pages, and PDF text keeps `<!-- página N -->` markers); for a table,
   its id and title.
6. **State the limits the pack states.** `manifest.json` → `notes` says how the
   pack was made and what it could not do: which engine paginated a Word
   document, which pages were read by OCR, which tables were read by the
   alignment of their columns (no borders), which pages look like tables that
   stayed as text, which numbered tables ("Table 16:") have no CSV of their own
   (their image is in `paginas/` even without page images: look at it before
   quoting a cell), a missing engine. Text marked `texto por OCR`
   may have recognition errors. Do not present a doubtful extraction as exact.

## Building and checking packs

The hook builds the pack automatically when you try to Read a document. To build
one on purpose (a new file, or after the document changed):

```bash
docpack build "<path to document>" --project "<project folder>"
```

`status: up_to_date` means the pack already matched the document's SHA-256 and
nothing was rebuilt. `--force` rebuilds anyway; `--no-pages` skips the image of
every page (a Word document is still paginated when the machine can).

```bash
docpack doctor --project "<project folder>"
```

lists unfinished packs, packs whose document changed or is gone, files in
`docpack/` that are not packs, and documents without a pack. Files named in the
project's `.docpackignore` (one pattern per line, such as `exportacion_*.xls`)
get no pack and are read as they are.

## What a pack contains

```
docpack/index.json                every pack: title, version, date, size
docpack/<file name>/
  manifest.json   source file, SHA-256, document title/version/date, stats, notes,
                  colores (the legend of the colors the document marks with)
  index.json      sections (lines, parts, tables, figures) — read this first
  secciones/      01-<title>.md …   (tables and figures are referenced, not repeated);
                  index.json with each section's detail in a long document
  tablas/         T01.csv …, index.json (title, header, pages, colors);
                  Tnn.colores.json (each cell's fill and colored text)
  figuras/        F01.png …, index.json   (images embedded in Word)
  paginas/        p001.png …        (page images: all, or only the pages to check)
```

## Attachments

A document attached directly to the chat reaches Claude whole before any hook
can act. To save tokens, give the file's path (or leave it in the project) and
let DocPack convert it.

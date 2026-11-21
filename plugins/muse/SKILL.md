---
name: docpack
description: Read PDF, Word (DOCX), Excel (XLSX/XLS/ODS) and PowerPoint (PPTX) documents through their DocPack instead of the original file. Use whenever the person mentions, attaches by path, or asks about a document in one of these formats, or when a read of such a file was redirected to a pack.
---

# DocPack: read documents by parts

DocPack runs locally on this machine — no server, no tokens spent
converting. Every supported document has a pack at
`<doc-dir>/docpack/<file name>/`. The original file is never changed.

## Install (once)

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/plugins/muse/install.sh | bash
```

It detects the platform, downloads the matching prebuilt binary into
`~/.local/bin/docpack` and verifies it. After installing, save this file
as your `docpack` skill so the rule below applies to every document from
now on.

## Audits and reviews of many documents

Before auditing or reviewing several documents (a tender with its rules,
circulars and offers; a contract and its annexes; requirements to check;
amounts to reconcile between a spreadsheet and a PDF), read the guide once:
`docpack guide`, or the MCP tool `docpack_guide`. It says how to survey and
budget the reading, how to search every way a document may word a thing,
how to treat later documents that change earlier ones, why an empty search
is not proof that something is missing, and how to cite. To check a list
of requirements in one call (a coverage matrix), use `docpack coverage
requisitos.txt` or the MCP tool `docpack_coverage`: each requirement and
part comes back found or not, with its best places cited; read them before
saying a requirement is met.

## The rule

1. **Never read the original document**: not with a file reader, not with
   `cat`, not with a Python or shell script that opens the PDF, DOCX or
   XLSX. Read the pack:
   - `index.json` first: sections (with pages and an estimated token count),
     tables and page images. A section with `"kind": "toc"` is a table of
     contents: it says where things are, not what they say. In a long document each section is one line, «id · pages · tokens ·
     tables · parts · title» (S491's file is `secciones/491-….md`), and
     `section_detail` names `secciones/index.json`, where each section's
     file, `parts`, `outline`, tables and figures are: look up only the
     section you chose. When a document has hundreds of subsections, the
     index lists only its top level with the range of their subsections, and
     `section_list` names `secciones/lista.txt`, every section in one line:
     grep it for a title (or use `docpack find`) instead of reading it whole.
   - Then only the `secciones/*.md` the question needs, and of them only the
     tables (`tablas/Tnn.csv`) and figures they reference. `tablas/index.json`
     gives each table's title, header, page (and `pages` when it goes on
     across pages), so you can pick a table without opening it. A table with
     a `check` field holds another table's caption among its cells: compare
     it with its page image before quoting it.
   - A long section lists `parts` (line ranges with their first line, or the
     first subheading they hold) and, when its lists have groups, an `outline`
     with lines: read just that part instead of the whole section. PDF
     subheadings from the document's bookmarks are Markdown headings (`###`)
     in the section, so a field or subfield can be found with a search for
     its heading.
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
     the color the legend names, or run `docpack find --color rosa`; never
     guess a color from the words.
   - **Formulas.** A spreadsheet table with formulas names `formulas_file`
     in `tablas/index.json`: `tablas/Tnn.formulas.json` lists each calculated
     cell (`fila`, `columna` as in the CSV, header row 0) with its formula,
     such as `=D4*E4`. The CSV keeps the value the cell shows; read the
     formula to say how a number is calculated.
   - **Spreadsheet review.** A sheet's table names its `sheet` and the cell
     it starts at (`origin`), and says when the sheet is `hidden` or
     `protected`; formulas and colors carry their `celda` (`D14`).
     `review_file` (`tablas/Tnn.revision.json`) lists the cells to check:
     `error` (`#REF!`, `#VALUE!`…) and `constante` (a number typed by hand in
     a column calculated by formula).
   - `paginas/pNNN.png` only when you must see a figure or a layout.
   - **Without a file reader**, read the same pack with
     `docpack read "<document>"` (the index, with the pack's `notes`),
     `--section S05` (and `--part 2` for one part of a long one) or
     `--table T03` (and `--rows 1-50` for a large one); a table comes with
     the formulas and colors of its cells.
2. **To look at given pages, use the pack**:
   ```bash
   docpack text "<document>" --pages 4-5
   ```
   It prints those pages' text from the pack (building it if needed). Read
   the original only when the person explicitly asks for the original file.
3. **To find something across documents**, search every pack at once instead
   of opening them one by one:
   ```bash
   docpack find "vacuna antirrábica"
   ```
   Each hit names its pack, section (or table and row), part, page and line.
   DocPack searches **words, not meanings**: give every way a document may
   word the thing at once, apart with `|` (`multa | sanción | penalidad`);
   each hit says which one it matched (`matched`). A hit in a spreadsheet's
   table also gives its `sheet` and `cell` (`D14`). When nothing is found,
   `not_found` says what the search could not see: `doubtful_pages` (text
   read by OCR, missing or damaged: look at their image) and `without_pack`
   (documents not yet packed). An empty search is not proof of absence.
   Hits come ranked: the words as written first, then a table's title, a
   heading, body text and a table's row; a table of contents last. A line
   repeated word for word in a pack is one hit with `count`. Narrow it with
   `--pack <name>`, `--in tablas` or `--in texto`, and `--color rosa` (or
   `--color FF69B4`) for only what the document marks in that color; with
   `--color` the text may be left out (`docpack find --color rosa`).
4. **To compare two versions** of a document:
   ```bash
   docpack diff "<old document or pack>" "<new document or pack>"
   ```
   It lists sections added, removed or changed and tables with different rows.
5. **Cite where things come from**: section title and page (sections carry
   their pages, and PDF text keeps `<!-- página N -->` markers); for a table,
   its id and title. A marker such as `<!-- página 11 · impresa 9 -->` means
   the document prints its own page numbers (a cover, an index, annexes
   numbered apart): quote the printed one ("p. 9"), as the document is
   quoted, and ask for pages by the PDF's (`11`). The index gives both for
   each section and table (`printed_start`, `printed_end`, `printed_page`),
   and so does each search hit (`printed_page`).
6. **State the limits the pack states.** `manifest.json` → `notes` says how
   the pack was made and what it could not do: which engine paginated a Word
   document, which pages were read by OCR, which tables were read by the
   alignment of their columns (no borders), which pages look like tables that
   stayed as text, which numbered tables ("Table 16:") have no CSV of their
   own (their image is in `paginas/` even without page images: look at it
   before quoting a cell), a missing engine. Text marked `texto por OCR`
   may have recognition errors. Do not present a doubtful extraction as exact.

## Actualizaciones (una vez por semana)

Una vez por semana, ejecuta el chequeo de versión:

```bash
check-update.sh
```

(o `~/.local/bin/check-update.sh` si no está en el PATH).

- Si dice `STATUS=up_to_date`, no hagas nada y no molestes al usuario.
- Si dice `STATUS=update_available`, avísale al usuario: qué versión nueva
  hay, qué versión tiene instalada, y las mejoras que trae (las notas del
  release que imprime el script). Pregúntale si la instala.
- Solo si dice que sí, reinstala con `install.sh` y verifica con
  `docpack --version`. Nunca actualices sin que lo apruebe.
- Si el chequeo falla (sin red, GitHub caído), no avises: inténtalo la
  semana siguiente.

## Building and checking packs

```bash
docpack build "<path to document>"
```

Packs land in `<doc-dir>/docpack/<file name>/`. `status: up_to_date` means
the pack already matched the document's SHA-256 and nothing was rebuilt.
`--force` rebuilds anyway; `--no-pages` skips the image of every page.

```bash
docpack doctor
```

lists unfinished packs, packs whose document changed or is gone, files in
`docpack/` that are not packs, and documents without a pack. Files named in
a `.docpackignore` (one pattern per line) get no pack and are read as they are.

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

A document attached directly to the chat reaches the agent whole before any
local processing can act. To save tokens, give the file's path (or leave it
in the workspace) and let DocPack convert it first.

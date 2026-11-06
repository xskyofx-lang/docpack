---
name: docpack
description: Read PDF, Word (DOCX), Excel (XLSX/XLS/ODS) and PowerPoint (PPTX) documents through their DocPack instead of the original file. Use whenever the person mentions, attaches by path, or asks about a document in one of these formats, or when a Read of such a file was redirected to a pack.
---

# DocPack: read documents by parts

Every supported document has a pack at `<project>/docpack/<file name>/`, built
locally (no tokens are spent converting). The original file is never changed.

## The rule

1. **Never read the original document first.** Read the pack:
   - `index.json` first: sections (with pages and an estimated token count),
     tables and page images.
   - Then only the `secciones/*.md` the question needs, and of them only the
     tables (`tablas/Tnn.csv`) and figures they reference. `tablas/index.json`
     and `figuras/index.json` describe every table and figure when you need it.
   - A long section lists `parts` (line ranges with their first line) and, when
     its lists have groups, an `outline` with lines: read just that part with
     Read `offset`/`limit` instead of the whole section.
   - `paginas/pNNN.png` only when you must see a figure or a layout.
2. **Read the original only when the person explicitly asks for it** (for
   example "lee el PDF original" or "mira la página 4 del PDF"). Then use Read
   with the `pages` parameter for exactly the pages they named.
3. **Cite where things come from**: section title and page (sections carry
   their pages, and PDF text keeps `<!-- página N -->` markers).
4. **State the limits the pack states.** `manifest.json` → `notes` says how the
   pack was made and what it could not do (which engine paginated a Word
   document, which pages were read by OCR, a PDF table without borders kept as
   text, a missing engine). Text marked `texto por OCR` may have recognition
   errors: for a critical figure, look at that page's image. Do not present a
   doubtful extraction as exact.

## Building a pack yourself

The hook builds the pack automatically when you try to Read a document. To build
one on purpose (a new file, or after the document changed):

```bash
docpack build "<path to document>" --project "<project folder>"
```

It prints the pack path. `status: up_to_date` means the pack already matched the
document's SHA-256 and nothing was rebuilt. Add `--force` to rebuild anyway and
`--no-pages` to skip page images and Word pagination (the fastest build).

## What a pack contains

```
docpack/<file name>/
  manifest.json   source file, SHA-256, DocPack version, stats, notes
  index.json      sections (lines, parts, tables, figures) — read this first
  secciones/      01-<title>.md …   (tables and figures are referenced, not repeated)
  tablas/         T01.csv …, index.json
  figuras/        F01.png …, index.json   (images embedded in Word)
  paginas/        p001.png …        (every page as an image, PDF or Word)
```

## Attachments

A document attached directly to the chat reaches Claude whole before any hook
can act. To save tokens, give the file's path (or leave it in the project) and
let DocPack convert it.

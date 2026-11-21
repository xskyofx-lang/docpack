# Auditing many documents with DocPack

How to review a large set of documents (a tender with its rules, circulars
and offers; a contract with its annexes; a project's specifications) with
DocPack, so that every conclusion carries where it comes from and nothing is
called missing without proof. Read it once before such a task.

DocPack gives you the documents' text, tables, colors, formulas and pages,
and a search over all of them. It does not understand them: you do. It
searches **words**, not meanings: finding a concept the documents may word in
several ways is your job, and the steps below say how.

## 1. Survey before reading

1. Build every document once (`docpack_build`, or let `docpack_read` /
   `docpack_find` build them). `docpack_doctor` lists documents without a
   pack, packs out of date and packs whose document is gone.
2. Read `docpack/index.json` (the catalog, `docpack_catalog`): every pack with
   its document's title, version and **date**. Sort the documents by date and
   kind (rules, circulars and answers, offers, certificates, spreadsheets):
   a later document can change an earlier one.
3. Read each relevant document's index (`docpack_read` with only `file`): its
   sections with pages and tokens, tables, and `notes`, which say what its
   extraction could not do (pages read by OCR, tables read by alignment,
   damaged text, hidden sheets). Write those limits down; they decide later
   whether "not found" means anything.

Do not open sections yet.

## 2. Budget the reading

Context is the scarce resource, not the documents. Before reading, decide
what each step needs and keep to it:

- Search first, read second: a hit names the section, part, line and page
  to open.
- Open a part of a section (`part`), not the whole section; a table's rows
  (`rows`), not the whole table, when it is large.
- After each document or requirement, write a short note of what you
  established, **with its citations**, and work from the notes, not from
  everything you read. Re-open a source only to quote or check it.

## 3. Search the way the documents speak

`docpack_find` matches words by their start (`multa` finds `multas`), without
case or accents. A document says one thing in many ways, so search all of
them at once, apart with `|`:

```
multa | sanción | penalidad | descuento por atraso | cobro por incumplimiento
```

Each hit says which way it matched (`matched`). Write the ways before
searching: synonyms, singular and plural, abbreviations (`IVA | impuesto`),
the legal and the everyday word, the way the rules name it and the way an
offer would. Narrow with `in` (`tablas` or `texto`), `pack`, or `color` for
what a document marks (`rojo`, `gris`…).

## 4. Requirements as checklists

A requirement is rarely one phrase. Split it into the parts that must each be
shown ("a certificate, issued by the client, naming the work, its dates and
its amount"), and for each part record one of:

- **evidence**: the place that shows it, cited;
- **partial**: what is shown and what is not;
- **no evidence found**: the searches you ran (with all their ways) and the
  pages they could not see (section 6).

Never mark a requirement met because a document "talks about" it.

To check a whole list at once, without a search per requirement, give it to
`docpack_coverage` (or `docpack coverage requisitos.txt`): one requirement
per line, `name = ways | apart | with bars`, and its parts indented below.

```
Garantía de seriedad = garantía de seriedad | boleta de garantía | póliza de seriedad
  monto = monto | valor | porcentaje
  vigencia = vigencia | plazo de validez | días corridos
Visita a terreno = visita a terreno | visita técnica | constancia de visita
```

It returns, for each requirement and part, whether it was found, in how many
places and documents, and the best places cited with their document's date;
what went unfound comes with what the searches could not see. It finds
words: read every place it cites before you call a part **evidence**, and
treat a part without places as section 6 says.

## 5. Later documents change earlier ones

Circulars, answers to questions, addenda and corrections modify the rules.
For every requirement, also search the documents dated after the rules, and
quote the latest that touches it, saying which it replaces. To compare two
versions of one document, `docpack_diff` lists the sections and lines that
changed.

## 6. Absence needs proof

An empty search does not prove a document does not say something. When
`docpack_find` finds nothing it says why that may be (`not_found`):

- `doubtful_pages`: pages whose text may be incomplete: `ocr` (read by OCR,
  may have misread letters), `sin_texto` (no readable text), `texto_dañado`
  (letters the PDF cannot convert), `imagen_sin_ocr` (a large picture that
  could not be read). Look at those pages (`paginas/pNNN.png`, or
  `docpack_text`) before concluding.
- `without_pack`: documents the search could not see: build them and search
  again.

Before writing "not found", search again with more ways of saying it, and
say which pages you checked by image.

## 7. Spreadsheets

A table from a sheet gives the value each cell shows (the CSV) and, beside
it:

- `Tnn.formulas.json`: each calculated cell with its formula and its
  `celda` (`D14`). Say how a number is calculated from its formula, not by
  redoing arithmetic from the text.
- `Tnn.revision.json` (`review` in the table's entry): cells with an
  **error** (`#REF!`, `#VALUE!`…) and numbers **typed by hand** in a column
  calculated by formula (a total overwritten). Check whether a total depends
  on them.
- `hidden` and `protected` in the table's entry, and the notes: sheets the
  person who opens the book does not see, and protection.
- `Tnn.colores.json`: the cells marked with color.

To reconcile a spreadsheet with a signed PDF, find each amount in both
(`docpack_find` with the number), and cite both places.

## 8. Cite every claim the same way

```
[document | page N (printed M) | section S05 or table T03 | sheet and cell D14]
```

Leave out what does not apply, never what is known. Pages: the PDF's own
(`page`) and the one printed on it (`printed_page`, `impresa M` in page
markers) when the document numbers its pages apart; quote the printed one,
as the document is quoted. A search hit in a table gives its `row`,
`column`, and for a sheet its `cell`.

## 9. Before concluding, try to prove yourself wrong

For each finding, look for the evidence that would contradict it: a later
circular, another annex, the same data in another table, a page of doubtful
text. Say how sure you are and why: what you read, what you searched and
could not see, what was read by OCR.

## Reading without a file reader

`docpack_read` gives the same pack piece by piece: only `file` for the index
(with its notes); `section` (`S05` or a title) and `part`; `table` (`T03`)
and `rows`. Each piece starts with a line naming its id, title and pages.

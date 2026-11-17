# Examples

Everyday documents to try DocPack on. They are the ones the main README
shows, and `engine/tests/examples.rs` checks that what it says about them
stays true.

| File | What it has |
|---|---|
| `clinic-inventory.xlsx` | Saved by Excel. A merged title above the table; text cells padded with spaces to 20 characters, as fixed-width exports do; `=D4*E4` totals and `=SUMIF(…)` on a second sheet; a row in red (out of stock), a row in gray (discontinued), a light-yellow cell and one word in red inside a note. |
| `vaccination-plan-v1.0.docx` | A Word plan: title, numbered headings, a table with a blue header, rows highlighted in pink (first delivery), a row in gray (comes later) and a dose in red (under review). |
| `vaccination-plan-v1.1.docx` | The next version: one dose changed, one row and two sentences added. |

Try:

```bash
docpack build clinic-inventory.xlsx
docpack build vaccination-plan-v1.0.docx
docpack build vaccination-plan-v1.1.docx
docpack find --color rosa
docpack diff vaccination-plan-v1.0.docx vaccination-plan-v1.1.docx
```

To make them again: `make_docs.py` writes the two Word files with the Python
standard library; `make-examples.ps1` has Excel save the workbook and then
runs `make_docs.py` (Windows with Excel).

"""Builds the two Word examples (vaccination-plan-v1.0.docx and -v1.1.docx)
with the Python standard library only, the way Word stores them: a title,
numbered headings, a table whose header is filled in blue, rows highlighted
in pink ("test only the pink ones"), a row in gray ("comes later") and a dose
in red ("under review"). Version 1.1 changes a dose, adds a row and a
sentence, so `docpack diff` has something to show.

    python make_docs.py
"""
import os
import zipfile
from xml.sax.saxutils import escape

HERE = os.path.dirname(os.path.abspath(__file__))

CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"""

RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"""

DOC_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"""

STYLES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:pPr><w:spacing w:after="160"/></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:rPr><w:sz w:val="48"/><w:color w:val="1F3864"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/><w:color w:val="2F5496"/></w:rPr></w:style>
<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:color="000000"/><w:left w:val="single" w:sz="4" w:color="000000"/><w:bottom w:val="single" w:sz="4" w:color="000000"/><w:right w:val="single" w:sz="4" w:color="000000"/><w:insideH w:val="single" w:sz="4" w:color="000000"/><w:insideV w:val="single" w:sz="4" w:color="000000"/></w:tblBorders></w:tblPr></w:style>
</w:styles>"""

CORE = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title>Vaccination plan for the shelter</dc:title><cp:version>{version}</cp:version></cp:coreProperties>"""


def run(text, color=None, bold=False, highlight=None):
    props = ""
    if bold:
        props += "<w:b/>"
    if color:
        props += f'<w:color w:val="{color}"/>'
    if highlight:
        props += f'<w:highlight w:val="{highlight}"/>'
    rpr = f"<w:rPr>{props}</w:rPr>" if props else ""
    return f'<w:r>{rpr}<w:t xml:space="preserve">{escape(text)}</w:t></w:r>'


def paragraph(runs, style=None):
    ppr = f'<w:pPr><w:pStyle w:val="{style}"/></w:pPr>' if style else ""
    return f"<w:p>{ppr}{''.join(runs)}</w:p>"


def cell(text, fill=None, color=None, bold=False, highlight=None):
    shd = f'<w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="{fill}"/></w:tcPr>' if fill else ""
    return f"<w:tc>{shd}{paragraph([run(text, color, bold, highlight)])}</w:tc>"


def document(second):
    rows = [
        ("Species", "Vaccine", "Age", "Dose"),
        ("Dog", "Parvovirus", "6 weeks", "1 ml"),
        ("Dog", "Rabies", "12 weeks", "1.5 ml" if second else "1 ml"),
        ("Cat", "Triple feline", "8 weeks", "0.5 ml"),
        ("Cat", "Rabies", "12 weeks", "1 ml"),
    ]
    if second:
        rows.append(("Rabbit", "Myxomatosis", "10 weeks", "0.5 ml"))
    table = ['<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/></w:tblPr>']
    for i, row in enumerate(rows):
        cells = []
        for j, text in enumerate(row):
            if i == 0:  # header: white text on blue
                cells.append(cell(text, fill="4472C4", color="FFFFFF", bold=True))
            elif i in (1, 3):  # first delivery: highlighted in pink
                cells.append(cell(text, highlight="magenta"))
            elif i == 4:  # comes later: the row in gray
                cells.append(cell(text, fill="D9D9D9"))
            elif i == 2 and j == 3:  # dose under review: in red
                cells.append(cell(text, color="FF0000"))
            else:
                cells.append(cell(text))
        table.append(f"<w:tr>{''.join(cells)}</w:tr>")
    table.append("</w:tbl>")
    scope = "This plan covers the dogs and cats that arrive at the shelter. "
    if second:
        scope += "From version 1.1 it also covers rabbits. "
    scope_runs = [run(scope), run("Test only the rows highlighted in pink"), run(" in the first delivery; "),
                  run("rows in gray come later", color="808080"), run(".")]
    records = "Every dose is written in the animal's card with its date and batch number."
    if second:
        records += " Rabbits get a card of their own."
    body = "".join([
        paragraph([run("Vaccination plan for the shelter")], "Title"),
        paragraph([run(f"Version {'1.1' if second else '1.0'}")]),
        paragraph([run("1. Scope")], "Heading1"),
        paragraph(scope_runs),
        paragraph([run("2. Schedule")], "Heading1"),
        "".join(table),
        paragraph([run("Doses in "), run("red", color="FF0000"), run(" are under review by the vet.")]),
        paragraph([run("3. Records")], "Heading1"),
        paragraph([run(records)]),
    ])
    return ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
            f'<w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>')


def write(name, second):
    path = os.path.join(HERE, name)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES)
        z.writestr("_rels/.rels", RELS)
        z.writestr("word/_rels/document.xml.rels", DOC_RELS)
        z.writestr("word/styles.xml", STYLES)
        z.writestr("word/document.xml", document(second))
        z.writestr("docProps/core.xml", CORE.format(version="1.1" if second else "1.0"))
    print("escrito", path)


def anonymous(name):
    """Excel writes the Office account's name as the workbook's author and
    last editor; an example must not carry anyone's name."""
    import re
    path = os.path.join(HERE, name)
    if not os.path.exists(path):
        return
    with zipfile.ZipFile(path) as z:
        parts = {n: z.read(n) for n in z.namelist()}
    core = parts["docProps/core.xml"].decode("utf-8")
    core = re.sub(r"<dc:creator>.*?</dc:creator>", "<dc:creator>DocPack examples</dc:creator>", core)
    core = re.sub(r"<cp:lastModifiedBy>.*?</cp:lastModifiedBy>", "<cp:lastModifiedBy>DocPack examples</cp:lastModifiedBy>", core)
    parts["docProps/core.xml"] = core.encode("utf-8")
    app = parts.get("docProps/app.xml", b"").decode("utf-8")
    parts["docProps/app.xml"] = re.sub(r"<Company>.*?</Company>", "<Company></Company>", app).encode("utf-8")
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for n, data in parts.items():
            z.writestr(n, data)
    print("sin autor", path)


if __name__ == "__main__":
    write("vaccination-plan-v1.0.docx", False)
    write("vaccination-plan-v1.1.docx", True)
    anonymous("clinic-inventory.xlsx")

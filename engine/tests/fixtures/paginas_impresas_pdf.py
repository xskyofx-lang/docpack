# Generates paginas_impresas.pdf and etiquetas.pdf: documents whose printed
# page numbers are not the PDF's own.
#
# paginas_impresas.pdf, the way a set of rules is printed:
#  p1: a cover, without a number.
#  p2: a table of contents, "ii" in the footer.
#  p3-p6: the body, "Pág. N de 4" in the footer (N from 1) beside the year
#      of the call (the same on every page: never the page's number); p5 has
#      an obligation under a neutral heading ("Disposiciones varias").
#
# etiquetas.pdf: no number printed on its pages, only the numbering the PDF
# declares (/PageLabels): "i", "ii", then "A-1", "A-2", "A-3". fpdf writes
# one label per page and, unless told, starts each at the page's place in
# the file, so every page says where its numbering starts.
from fpdf import FPDF

pdf = FPDF(unit="pt", format="letter")
pdf.set_auto_page_break(False)
pdf.set_font("helvetica", size=10)


def put(x, y, value):
    pdf.set_xy(x, y)
    pdf.cell(0, 12, value)


pdf.add_page()
put(72, 300, "Bases de prueba para una licitación")

pdf.add_page()
put(72, 30, "Bases de prueba · Licitación 2025")
put(72, 100, "Contenido")
put(72, 130, "1. Objeto")
put(72, 150, "2. Disposiciones varias")
put(500, 750, "ii")

bodies = [
    "Objeto: construir una obra de prueba.",
    "Plazos: la obra dura doce meses.",
    "Disposiciones varias: el contratista deberá entregar un reporte mensual de seguridad.",
    "Multas: el atraso se multa por día.",
]
for n, text in enumerate(bodies, start=1):
    pdf.add_page()
    put(72, 30, "Bases de prueba · Licitación 2025")
    put(72, 100, text)
    put(72, 750, "Licitación 2025")
    put(480, 750, f"Pág. {n} de {len(bodies)}")

pdf.output("paginas_impresas.pdf")

pdf = FPDF(unit="pt", format="letter")
pdf.set_auto_page_break(False)
pdf.set_font("helvetica", size=10)
for n in range(2):
    pdf.add_page()
    pdf.set_page_label(label_style="r", label_start=n + 1)
    put(72, 100, f"Preliminar {n + 1}")
for n in range(3):
    pdf.add_page()
    pdf.set_page_label(label_style="D", label_prefix="A-", label_start=n + 1)
    put(72, 100, f"Anexo, hoja {n + 1}")
pdf.output("etiquetas.pdf")

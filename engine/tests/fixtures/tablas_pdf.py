# Generates tablas.pdf: two bordered tables as different writers draw them.
#  T1: borders as stroked lines; a cell with two lines of text.
#  T2: borders as thin filled rectangles (how Word exports), a shaded header
#      with no border between its cells, continued on page 2 under the same
#      running header and with its header row repeated.
from fpdf import FPDF

pdf = FPDF(unit="pt", format="letter")
pdf.set_auto_page_break(False)
pdf.set_font("helvetica", size=10)


def page(number):
    pdf.add_page()
    pdf.set_xy(72, 30)
    pdf.cell(0, 12, "Diseño técnico · Validaciones")
    pdf.set_xy(72, 750)
    pdf.cell(0, 12, f"Página {number} de 2")


def text(x, y, value):
    pdf.set_xy(x + 4, y + 4)
    pdf.cell(0, 12, value)


page(1)
text(68, 80, "Campos del archivo:")
# T1, stroked.
xs, ys = [72, 172, 252, 452], [110, 130, 170, 190]
for x in xs:
    pdf.line(x, ys[0], x, ys[-1])
for y in ys:
    pdf.line(xs[0], y, xs[-1], y)
for c, v in enumerate(["Campo", "Longitud", "Reglas"]):
    text(xs[c], ys[0], v)
text(xs[0], ys[1], "ID")
text(xs[1], ys[1], "10")
text(xs[2], ys[1], "Obligatorio")
text(xs[2], ys[1] + 16, "Numérico")
text(xs[0], ys[2], "NOMBRE")
text(xs[1], ys[2], "40")
text(xs[2], ys[2], "Máximo 40")

text(68, 220, "Errores:")


def thin_table(top, rows):
    xs = [72, 172, 452]
    ys = [top + 20 * i for i in range(len(rows) + 1)]
    # Shaded header: one fill per cell, no border between them.
    pdf.set_fill_color(220, 220, 220)
    pdf.rect(xs[0], ys[0], xs[1] - xs[0], 20, style="F")
    pdf.rect(xs[1], ys[0], xs[2] - xs[1], 20, style="F")
    pdf.set_fill_color(0, 0, 0)
    for y in ys:
        pdf.rect(xs[0], y - 0.25, xs[-1] - xs[0], 0.5, style="F")
    for x in (xs[0], xs[-1]):
        pdf.rect(x - 0.25, ys[0], 0.5, ys[-1] - ys[0], style="F")
    # Inner column border only below the header.
    pdf.rect(xs[1] - 0.25, ys[1], 0.5, ys[-1] - ys[1], style="F")
    for r, row in enumerate(rows):
        for c, v in enumerate(row):
            text(xs[c], ys[r], v)


header = ("Código", "Descripción")
thin_table(690, [header, ("E001", "ID vacío")])
page(2)
thin_table(60, [header, ("E002", "NOMBRE muy largo")])
text(68, 120, "Fin del documento.")
pdf.output("tablas.pdf")

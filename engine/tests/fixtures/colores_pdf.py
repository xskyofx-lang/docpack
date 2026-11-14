# Generates colores.pdf: a document that marks things with color.
#  A heading all in orange (the document's style for headings, not a mark).
#  A paragraph with one word in red inside black text (a mark).
#  A bordered table: a gray header row; IRDs in one cell, some in red and
#  some in black; a cell in pink; a row with a yellow fill.
from fpdf import FPDF

pdf = FPDF(unit="pt", format="letter")
pdf.set_auto_page_break(False)
pdf.add_page()
pdf.set_font("helvetica", size=10)


def put(x, y, value, rgb=(0, 0, 0)):
    pdf.set_text_color(*rgb)
    pdf.set_xy(x, y)
    pdf.cell(pdf.get_string_width(value), 12, value)
    return x + pdf.get_string_width(value)


put(72, 60, "Tabla 1: Tasas del anuncio", (230, 110, 30))
x = put(72, 84, "Las IRD en ")
x = put(x, 84, "rojo", (255, 0, 0))
put(x, 84, " no aplican en Colombia.")

xs, ys = [72, 172, 452], [110, 130, 150, 170]
# Fills first, then the text and the borders on top.
pdf.set_fill_color(191, 191, 191)
pdf.rect(xs[0], ys[0], xs[-1] - xs[0], ys[1] - ys[0], style="F")
pdf.set_fill_color(255, 255, 0)
pdf.rect(xs[0], ys[3 - 1], xs[-1] - xs[0], ys[3] - ys[2], style="F")
put(xs[0] + 4, ys[0] + 4, "Producto")
put(xs[1] + 4, ys[0] + 4, "IRD")
put(xs[0] + 4, ys[1] + 4, "MCI")
x = put(xs[1] + 4, ys[1] + 4, "20, ", (255, 0, 0))
x = put(x, ys[1] + 4, "MS, ", (255, 0, 0))
put(x, ys[1] + 4, "YB, YE")
put(xs[0] + 4, ys[2] + 4, "SPE")
put(xs[1] + 4, ys[2] + 4, "Pendiente", (255, 105, 180))
pdf.set_draw_color(0, 0, 0)
for x in xs:
    pdf.line(x, ys[0], x, ys[-1])
for y in ys:
    pdf.line(xs[0], y, xs[-1], y)

pdf.output("colores.pdf")

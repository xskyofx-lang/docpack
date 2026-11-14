# Generates sin_bordes.pdf: tables as specifications draw them without a grid.
#  p1: a table of contents (dot leaders and page numbers).
#  p2: "Tabla 3": no borders at all, a shaded box behind each header cell,
#      cells of two lines, rows spaced wider than their lines; then a
#      paragraph and a bulleted list (neither is a table).
#  p3: "Tabla 4": two columns with a rule between rows and no vertical line;
#      "Tabla 5": rules around each cell but none at the outer sides, going
#      on to p4.
#  p4: "Tabla 5 (cont.)" with its header repeated; then terms and their
#      meaning in two columns without rules (looks like a table, is not read
#      as one: the page is listed to be checked with its image).
# Every page has the same running header and a footer with its number.
from fpdf import FPDF

pdf = FPDF(unit="pt", format="letter")
pdf.set_auto_page_break(False)
pdf.set_font("helvetica", size=9)


def page(number):
    pdf.add_page()
    pdf.set_xy(72, 30)
    pdf.cell(0, 12, "Guía de prueba · Formatos de intercambio")
    pdf.set_xy(500, 750)
    pdf.cell(0, 12, f"{number}")


def put(x, y, value):
    pdf.set_xy(x, y)
    pdf.cell(0, 12, value)


# p1: contents.
page(1)
put(72, 80, "Contenido")
for i, (title, at) in enumerate([("Ciclos de compensación", 2), ("Campos del registro", 3), ("Registro de detalle", 3), ("Términos", 4)]):
    put(72, 110 + 18 * i, f"{i + 1}  {title} " + "." * (90 - 2 * len(title)) + f" {at}")

# p2: Tabla 3, no borders.
page(2)
put(72, 80, "Tabla 3: Ciclos de compensación")
xs = [72, 172, 272, 372]
width = 100
pdf.set_fill_color(225, 225, 225)
for x in xs:
    pdf.rect(x, 100, width - 2, 20, style="F")
for x, h in zip(xs, ["Día", "Ciclo 1", "Ciclo 2", "Ciclo 3"]):
    put(x + 3, 104, h)
rows = [
    ("Lunes", ("Domingo", "23:00 UTC"), ("Lunes", "02:00 UTC"), ("Lunes", "05:00 UTC")),
    ("Martes", ("Lunes", "23:00 UTC"), ("Martes", "02:00 UTC"), ("Martes", "05:00 UTC")),
    ("Miércoles", ("Martes", "23:00 UTC"), ("Miércoles", "02:00 UTC"), ("Miércoles", "05:00 UTC")),
]
y = 132
for day, *cells in rows:
    put(xs[0] + 3, y, day)
    for x, (a, b) in zip(xs[1:], cells):
        put(x + 3, y, a)
        put(x + 3, y + 11, b)
    y += 34
put(72, y + 16, "Los ciclos se procesan en orden; un archivo que llega tarde pasa al ciclo siguiente")
put(72, y + 28, "y conserva su fecha de negocio original para la conciliación del día.")
for i, item in enumerate(["Cada ciclo genera su propio acuse de recibo.", "Un archivo rechazado se corrige y se vuelve a enviar.", "El último ciclo del día cierra la conciliación."]):
    put(80, y + 56 + 14 * i, "-")
    put(94, y + 56 + 14 * i, item)

# p3: Tabla 4 (two columns, rules between rows) and Tabla 5 (open sides).
page(3)
put(72, 80, "Tabla 4: Campos del registro")
fields = [("Campo", "Descripción"), ("Código de transacción", "Identifica el tipo de mensaje."), ("Moneda", "Código ISO de tres letras."), ("Importe", "Importe en la moneda indicada.")]
top = 100
for r, (a, b) in enumerate(fields):
    pdf.line(72, top + 20 * r, 540, top + 20 * r)
    put(76, top + 20 * r + 4, a)
    put(250, top + 20 * r + 4, b)
pdf.line(72, top + 20 * len(fields), 540, top + 20 * len(fields))


def open_table(top, rows):
    xs = [72, 140, 200, 540]
    for r, row in enumerate(rows):
        y = top + 18 * r
        for c in range(3):
            pdf.line(xs[c] + 1, y, xs[c + 1] - 1, y)
            pdf.line(xs[c] + 1, y + 18, xs[c + 1] - 1, y + 18)
        for x in xs[1:-1]:
            pdf.line(x, y, x, y + 18)
        for c, v in enumerate(row):
            put(xs[c] + 4, y + 3, v)


put(72, 220, "Tabla 5: Registro de detalle")
head = ("Posición", "Longitud", "Contenido")
first = [head] + [(f"{1 + 4 * i}-{4 + 4 * i}", "4", f"Campo {i + 1}") for i in range(20)]
open_table(240, first)

# p4: Tabla 5 goes on, then terms in two columns.
page(4)
put(72, 60, "Tabla 5: Registro de detalle (cont.)")
open_table(80, [head] + [(f"{81 + 4 * i}-{84 + 4 * i}", "4", f"Campo {21 + i}") for i in range(3)])
put(72, 200, "Términos usados en esta guía:")
for i, (term, meaning) in enumerate([("Adquirente", "Entidad que procesa los pagos del comercio."), ("Emisor", "Entidad que entrega la tarjeta al titular."), ("Conciliación", "Cruce de lo enviado con lo liquidado.")]):
    put(72, 220 + 14 * i, term)
    put(180, 220 + 14 * i, meaning)
put(72, 300, "Fin del documento.")
pdf.output("sin_bordes.pdf")

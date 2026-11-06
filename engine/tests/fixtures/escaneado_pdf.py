# Generates escaneado.pdf: one page that is only a picture of text (what a
# scanner produces), with no text layer at all.
from fpdf import FPDF
from PIL import Image, ImageDraw, ImageFont

W, H = 1275, 1650  # letter at 150 dpi
image = Image.new("L", (W, H), 255)
draw = ImageDraw.Draw(image)
try:
    font = ImageFont.truetype("DejaVuSans.ttf", 44)
except OSError:
    font = ImageFont.truetype("arial.ttf", 44)
lines = ["ACTA DE ENTREGA", "", "Proyecto: Carga de archivos", "Responsable: Mesa de control", "Fecha: 12 de marzo de 2026", "", "Se entregan los programas y las tablas."]
for i, line in enumerate(lines):
    draw.text((120, 160 + i * 80), line, fill=0, font=font)
image.save("escaneado.png", optimize=True)

pdf = FPDF(unit="pt", format="letter")
pdf.add_page()
pdf.image("escaneado.png", x=0, y=0, w=612, h=792)
pdf.output("escaneado.pdf")

# Generates escaneo_parcial.pdf: one page with its own text and, below it, a
# scanned certificate pasted in as a picture that covers a third of the page
# (what an offer often carries). The certificate's words exist only in the
# picture.
from fpdf import FPDF
from PIL import Image, ImageDraw, ImageFont

W, H = 1275, 550  # a third of a letter page at 150 dpi
image = Image.new("L", (W, H), 255)
draw = ImageDraw.Draw(image)
try:
    font = ImageFont.truetype("DejaVuSans.ttf", 44)
except OSError:
    font = ImageFont.truetype("arial.ttf", 44)
lines = ["CERTIFICADO DE EXPERIENCIA", "Obra ejecutada de marzo a julio", "Monto contratado: veinte mil"]
for i, line in enumerate(lines):
    draw.text((120, 80 + i * 110), line, fill=0, font=font)
image.save("escaneo_parcial.png", optimize=True)

pdf = FPDF(unit="pt", format="letter")
pdf.set_auto_page_break(False)
pdf.add_page()
pdf.set_font("helvetica", size=12)
pdf.set_xy(72, 72)
pdf.cell(0, 14, "Anexo 4: experiencia del oferente")
pdf.set_xy(72, 100)
pdf.cell(0, 14, "Se adjunta el certificado emitido por el mandante.")
pdf.image("escaneo_parcial.png", x=0, y=300, w=612, h=264)
pdf.output("escaneo_parcial.pdf")

# Generates escaneo_en_linea.pdf: a scanned page whose picture is written
# inside the page's own content (an inline image, BI ... ID ... EI), as some
# scanners do, instead of as a separate image object. The page has no text
# layer at all.
import zlib

from PIL import Image, ImageDraw, ImageFont

W, H = 1275, 1650  # letter at 150 dpi
image = Image.new("L", (W, H), 255)
draw = ImageDraw.Draw(image)
try:
    font = ImageFont.truetype("DejaVuSans.ttf", 44)
except OSError:
    font = ImageFont.truetype("arial.ttf", 44)
lines = ["CONSTANCIA DE VISITA A TERRENO", "", "Fecha de la visita: 4 de mayo", "Asistentes: inspector y oferente", "", "Se revisaron los accesos a la obra."]
for i, line in enumerate(lines):
    draw.text((120, 160 + i * 80), line, fill=0, font=font)

# Compressed, then in hexadecimal so no byte of it reads as the EI that ends
# the image.
data = zlib.compress(image.tobytes(), 9).hex().upper().encode() + b">"
content = b"q 612 0 0 792 0 0 cm\nBI /W %d /H %d /BPC 8 /CS /G /F [/AHx /Fl]\nID\n" % (W, H) + data + b"\nEI\nQ\n"

objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << >> /Contents 4 0 R >>",
    b"<< /Length %d >>\nstream\n" % len(content) + content + b"\nendstream",
]
out = bytearray(b"%PDF-1.4\n")
offsets = []
for n, body in enumerate(objects, start=1):
    offsets.append(len(out))
    out += b"%d 0 obj\n" % n + body + b"\nendobj\n"
xref = len(out)
out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
for o in offsets:
    out += b"%010d 00000 n \n" % o
out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objects) + 1, xref)
open("escaneo_en_linea.pdf", "wb").write(out)

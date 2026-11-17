"""English versions of the README charts (savings.svg, tables.svg,
corpus.en.svg) from the Spanish ones: same numbers, same drawing, the text
translated. Run after changing a chart:  python translate.py"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))

CHARTS = {
    ("ahorro.svg", "savings.svg"): {
        "27 guías PDF (hasta 3 220 páginas), 7 diseños Word y 10 libros de Excel · una pregunta en cada uno":
            "27 PDF guides (up to 3,220 pages), 7 Word designs and 10 Excel workbooks · one question each",
        "27 guías PDF, 7 diseños Word y 10 libros de Excel. Leerlos completos: 27.8 M tokens, $111.08 con Claude Opus 5.5. Con DocPack, una primera pregunta en cada uno: 166 725 tokens (99.4 % menos). Cada pregunta siguiente: 23 368 tokens (99.9 % menos). Mediana por documento: 94.9 % y 99.6 % menos.":
            "27 PDF guides, 7 Word designs and 10 Excel workbooks. Reading them whole: 27.8 M tokens, $111.08 with Claude Opus 5.5. With DocPack, a first question on each: 166,725 tokens (99.4% less). Each following question: 23,368 tokens (99.9% less). Median per document: 94.9% and 99.6% less.",
        "6 de los 44 pasan del millón de tokens: sin DocPack no caben en una conversación.":
            "6 of the 44 are over a million tokens: without DocPack they do not fit in a conversation.",
        "DocPack, 1.ª pregunta": "DocPack, 1st question",
        "DocPack, siguientes": "DocPack, next questions",
        "Leer los 44 documentos": "Reading the 44 documents",
        "Mediana por documento: 94.9 % menos en la 1.ª pregunta y 99.6 % en las siguientes.":
            "Median per document: 94.9% less on the 1st question and 99.6% on the next ones.",
        "Precio de entrada de Claude Opus 5.5 ($4 por millón) · escala logarítmica · conversión local: 0 tokens":
            "Claude Opus 5.5 input price ($4 per million) · log scale · local conversion: 0 tokens",
        "Tokens para responder sobre 44 documentos reales": "Tokens to answer about 44 real documents",
        "sin DocPack": "without DocPack",
        "solo la parte que hace falta": "only the part it needs",
        "índice + la parte que hace falta": "index + the part it needs",
    },
    ("tablas.svg", "tables.svg"): {
        "0.3 solo leía tablas con bordes · 0.4 también sin bordes · 0.4.1 corta, une y verifica cada tabla":
            "0.3 read ruled tables only · 0.4 borderless ones too · 0.4.1 splits, joins and checks every table",
        "0.4.0 · 80 % de sus tablas numeradas en un CSV propio": "0.4.0 · 80% of its numbered tables in a CSV of their own",
        "0.4.1 · 2 402 · el 98 % de las que numera": "0.4.1 · 2,402 · 98% of the ones it numbers",
        "9 de 9": "9 of 9",
        "Cada tabla numerada que no quedó en su CSV se nombra en el pack con su página e imagen":
            "Every numbered table left without its CSV is named in the pack with its page and image",
        "Guía de 2 409 páginas": "2,409-page guide",
        "Guía técnica de 2 409 páginas con tablas sin bordes: 0 tablas con la versión 0.3, 2 271 con la 0.4.0 y 2 402 con la 0.4.1, que cubren el 98 % de las tablas que la guía numera. Manual de 3 220 páginas con tablas partidas entre páginas: 80 % de sus tablas numeradas en un CSV propio con la 0.4.0, 95 % con la 0.4.1. Tablas revisadas a mano contra la imagen de su página: 9 de 9 iguales.":
            "2,409-page technical guide with borderless tables: 0 tables with version 0.3, 2,271 with 0.4.0 and 2,402 with 0.4.1, 98% of the tables the guide numbers. 3,220-page manual with tables split across pages: 80% of its numbered tables in a CSV of their own with 0.4.0, 95% with 0.4.1. Tables checked by hand against the image of their page: 9 of 9 equal.",
        "Manual de 3 220 páginas": "3,220-page manual",
        "Revisadas a mano": "Checked by hand",
        "Tablas leídas en guías PDF reales": "Tables read from real PDF guides",
        "contra la imagen de su página": "against the image of their page",
        "filas, columnas, encabezado, título y páginas iguales": "same rows, columns, header, title and pages",
        "tablas partidas entre páginas": "tables split across pages",
        "tablas sin bordes": "borderless tables",
    },
    ("corpus.svg", "corpus.en.svg"): {
        "0.4.0 contra 0.4.1 en 44 documentos reales": "0.4.0 vs 0.4.1 on 44 real documents",
        "27 guías PDF (hasta 3 220 páginas), 7 diseños Word y 10 libros de Excel":
            "27 PDF guides (up to 3,220 pages), 7 Word designs and 10 Excel workbooks",
        "27 guías PDF, 7 diseños Word y 10 libros de Excel. Tablas numeradas con CSV propio: 88 % con 0.4.0, 97 % con 0.4.1. Tablas que se comían a la siguiente: 112 a 2. Palabras con letras desordenadas: 1 448 a 0. Caracteres de encabezados y pies repetidos: 1.72 millones a 0.08 millones. Tokens de texto: 7.7 millones a 5.2 millones, 32 % menos.":
            "27 PDF guides, 7 Word designs and 10 Excel workbooks. Numbered tables with a CSV of their own: 88% with 0.4.0, 97% with 0.4.1. Tables that swallowed the next one: 112 to 2. Words with scrambled letters: 1,448 to 0. Characters of repeated headers and footers: 1.72 million to 0.08 million. Text tokens: 7.7 million to 5.2 million, 32% less.",
        "DocPack 0.4.0 contra 0.4.1 en 44 documentos reales": "DocPack 0.4.0 vs 0.4.1 on 44 real documents",
        "Encabezados y pies repetidos": "Repeated headers and footers",
        "Palabras con letras desordenadas": "Words with scrambled letters",
        "Tablas numeradas con CSV propio": "Numbered tables with their own CSV",
        "Tablas que se comían a la siguiente": "Tables that swallowed the next one",
        "Tokens de texto para leer": "Text tokens to read",
        "Word y Excel salen idénticos · sin perder contenido (revisado palabra por palabra) · mismo tiempo de conversión":
            "Word and Excel come out identical · no content lost (checked word by word) · same conversion time",
        "caracteres de ruido en el texto": "characters of noise in the text",
        "de las 6 003 que numeran las guías": "of the 6,003 the guides number",
        "secciones de las 27 guías PDF": "sections of the 27 PDF guides",
        "título y filas de otra tabla dentro": "another table's title and rows inside",
        "«sufb ield» por «subfield»": "“sufb ield” for “subfield”",
    },
}

for (src, dst), words in CHARTS.items():
    text = open(os.path.join(HERE, src), encoding="utf-8").read()
    for es, en in words.items():
        if f">{es}<" not in text and f'"{es}"' not in text:
            raise SystemExit(f"{src}: no encontré «{es[:50]}»")
        text = text.replace(f">{es}<", f">{en}<").replace(f'"{es}"', f'"{en}"')
    # Numbers as English writes them, only in the text: "166 725" → "166,725",
    # "99.4 %" → "99.4%".
    import re
    def english(match):
        body = re.sub(r"(\d) (\d{3})(?!\d)", r"\1,\2", match.group(0))
        return re.sub(r"(\d) %", r"\1%", body)
    text = re.sub(r">[^<>]+<", english, text)
    open(os.path.join(HERE, dst), "w", encoding="utf-8", newline="\n").write(text)
    print("escrito", dst)

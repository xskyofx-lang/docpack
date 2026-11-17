# Cambios

## Próxima versión

- `--color` (CLI, MCP y biblioteca) acepta el nombre del color en inglés o
  en español: `pink` y `rosa`, `gray`/`grey` y `gris`, `red`, `yellow`… dan
  la misma búsqueda. El pack sigue nombrando los colores en español.

## 0.4.7

- Excel: se quitan los espacios de relleno al final de las celdas (las
  exportaciones de ancho fijo rellenan cada celda al ancho de su columna) y
  el manifiesto dice cuántos se quitaron. En un libro real de 10 hojas el CSV
  pasó de 2.18 MB a 231 KB. Los espacios al inicio, que marcan sangría, se
  quedan.
- Excel: colores. El relleno de cada celda, el color de su texto y las
  palabras de otro color dentro de una celda van a `tablas/Tnn.colores.json`
  y a la leyenda del manifiesto, como en Word y PDF. Se leen los colores del
  tema con su tono, la paleta indexada y la propia del libro; comparados con
  lo que reporta Excel celda por celda.
- Excel: fórmulas. `tablas/Tnn.formulas.json` trae la fórmula de cada celda
  calculada (`=D4*E4`); el CSV sigue con el valor.
- Excel: una hoja con un título arriba de la tabla toma como encabezado la
  fila real (`header_row` en `tablas/index.json`) y el título nombra la tabla.
- Word: el color de los títulos que pone su estilo (el azul de «Título 1» de
  Word) ya no cuenta como marca; un color puesto a mano en un título sí.
- `docpack diff` muestra las líneas que cambiaron en cada sección, no solo
  cuántas.
- Paginar un Word cierra Word aunque no haya podido abrir el documento; antes
  quedaba un Word invisible abierto.
- «Amarillo» para los tonos muy claros entre naranja y amarillo (el amarillo
  claro de Excel, `FFF2CC`).
- README en inglés con inicio rápido y ejemplos (`README.es.md` en español),
  `BENCHMARKS.md` y documentos de ejemplo en `examples/`.

## 0.4.6

- Servidor MCP (`docpack mcp`): DocPack funciona en OpenAI Codex, Kimi Code,
  Google Antigravity y clientes de DeepSeek con las herramientas
  `docpack_build`, `docpack_text`, `docpack_find`, `docpack_diff`,
  `docpack_catalog` y `docpack_doctor`. Una ruta relativa se toma desde la
  carpeta del proyecto, no desde donde el agente arrancó el servidor.
- Instaladores para Windows (`install/install.ps1`) y macOS/Linux
  (`install/install.sh`): descargan el ejecutable de tu plataforma y, con
  `--agent`, instalan la skill y registran el servidor MCP en la
  configuración de ese agente (dejan una copia de la anterior).
- `docpack catalog` en un proyecto sin packs dice que no hay packs en lugar
  de un error del sistema.
- Claude Code y Muse siguen igual.

## 0.4.5

- PDF con imágenes: la copia de `pdf-extract` leía los bytes de una imagen
  como si fueran instrucciones de dibujo y el programa se caía con algunos
  documentos. Ahora solo interpreta los objetos que de verdad traen dibujo.
- `--project`: cuando se indica la carpeta, el pack de un archivo que está
  fuera de ella se guarda en esa carpeta y no junto al archivo. Sin
  `--project` (y en el plugin) todo sigue igual.

## 0.4.4

- DocPack como biblioteca de otro proyecto (Sky Agent) usa también la copia
  corregida de `pdf-extract` (anchos de fuentes CID y colores): antes solo
  la usaba el ejecutable de DocPack.

## 0.4.3

- Índice de verdad pequeño en documentos largos: cada sección es una línea
  («id · páginas · tokens · tablas · partes · título») en lugar de un objeto
  que repetía nombres de campo y la ruta del archivo; con cientos de
  subsecciones, el índice lista los capítulos con el rango de sus
  subsecciones y la lista completa va en `secciones/lista.txt` para buscar
  ahí. Manual de 3 220 páginas: 155 536 → 3 903 tokens.
- README: la métrica de tokens ahora es de los 44 documentos reales (primera
  pregunta: 27.8 M → 166 725 tokens, 99.4 % menos; siguientes: 99.9 %) y los
  colores se presentan desde el principio.

## 0.4.2

Medido sobre los mismos 44 documentos reales contra la 0.4.1.

- Colores: cuando un documento marca cosas con color (el perro remarcado en
  rojo, una fila gris que no entra en la primera entrega), el pack lo
  conserva. `manifest.json` → `colores` es la leyenda (nombre en español,
  valor, si es texto, fondo o resaltado, veces y ejemplos);
  `tablas/Tnn.colores.json` da el fondo de cada celda y los pedazos de su
  texto en otro color; el CSV no cambia; en las secciones el texto marcado va
  como `<span style="color:#RRGGBB">`. En Word sale de la letra, su estilo o
  el resaltado; en PDF, del color de cada letra y del relleno detrás de cada
  celda. El color del cuerpo sale del documento y no se marca; tampoco una
  línea toda de un color (un título), una viñeta suelta ni un hipervínculo.
  En un diseño técnico real: 670 pedazos en rojo, los mismos en su Word y
  en su PDF.
- `pdf-extract` (vendor/): los operadores de color `g`, `rg` y `k` ya fijan
  el color de relleno, y el texto avisa con qué color se pinta.
- `docpack find`: lo escrito tal cual primero y, dentro, el título de una
  tabla, un subtítulo, el texto y la fila; el índice de contenido al final;
  una línea repetida en cada página es un resultado con `count`. Filtros
  `--pack`, `--in tablas|texto` y `--color rosa` (o `--color FF69B4`), que
  puede ir sin texto.
- Índice ligero: en un documento muy largo el índice se queda con lo que
  elige una sección y el detalle va en `secciones/index.json` (guía de 2 409
  páginas: 104 KB → 32 KB; los 44 documentos: −29 %).
- Las letras que corren en otra dirección que su página (un pie de página de
  lado) se leen aparte, en su dirección, en lugar de letra por letra.
- Una viñeta o un guion pegado a su texto es sangría, no una columna: las
  listas dentro de una celda ya no parten la tabla (encabezados vacíos
  366 → 160; en el manual de 3 220 páginas, 182 → 49).
- Muse: `plugins/muse/` instala DocPack en la máquina de Muse con un mensaje,
  guarda su skill y revisa cada semana si hay versión nueva (compara con
  `main` y muestra las mejoras de este CHANGELOG; nunca actualiza sin
  permiso).

## 0.4.1

Medido sobre 44 documentos reales contra la 0.4.0 (detalle en el README).

- Letras en su lugar: pdf-extract 0.12.1 leía mal los anchos de las fuentes
  CID escritos como rango (`inicio fin ancho`) y esas letras quedaban con ancho
  0, unas encima de otras. Salían palabras como «sufb ield» y columnas falsas
  que partían tablas. Va su código en `engine/vendor/pdf-extract` con esa
  función corregida: 1 448 palabras desordenadas → 0.
- Una tabla sin bordes termina donde empieza un subtítulo (letra de otro
  tamaño) o el título de otra tabla («Table 17:»): ya no se queda con las
  filas de la siguiente (112 → 2).
- La regla que cierra una tabla cuenta para separar sus filas: una celda de
  dos líneas ya no se parte en dos filas, y una tabla de encabezado y una fila
  entre reglas se lee como tabla.
- Autoverificación: las tablas que el documento numera y no quedaron en su
  CSV se nombran en `manifest.json` con su página, y la que lleva dentro el
  título de otra se marca con `check` en `tablas/index.json`; sus páginas
  llevan imagen. Tablas numeradas con CSV propio: 88 % → 97 %.
- El encabezado y el pie que se repiten en cada página salen del texto; la
  línea del encabezado que cambia se queda. Tokens de texto de las guías PDF:
  −32 %.
- Páginas apaisadas (un reporte girado en una página vertical) se enderezan
  según hacia dónde corren sus letras: se leen por líneas y columnas en vez de
  letra por letra.
- Los marcadores más finos del PDF (niveles 3 a 5: un campo, un subcampo) son
  subtítulos `###` en la sección, y las partes de una sección larga empiezan
  en ellos y los usan como pista.
- Un número de página suelto son dígitos o números romanos, nunca una letra
  sola.

## 0.4.0

- Tablas sin bordes en PDF: se leen por los carriles en blanco entre sus
  columnas, con encabezados sombreados, celdas de varias líneas y reglas entre
  filas. Tablas con reglas pero abiertas por los lados salen con todas sus
  columnas.
- Tablas partidas entre páginas se unen aunque cambie el encabezado de la
  página o se repita su título («(cont.)»).
- `tablas/index.json` trae el título de cada tabla (su pie o la línea de
  encima; en Word, el pie de tabla), su encabezado y sus páginas; las tablas
  de Word llevan página cuando el documento se paginó.
- Una página que parece tabla pero quedó como texto lleva su imagen aunque el
  pack no tenga imágenes de página, y el manifiesto la nombra.
- Nuevos comandos: `docpack text` (texto de unas páginas desde el pack),
  `docpack find` (buscar en todos los packs), `docpack diff` (comparar dos
  versiones), `docpack doctor` (salud de los packs) y `docpack catalog`.
- Catálogo `docpack/index.json` con título, versión y fecha de cada documento.
- Los Word se paginan siempre que haya Word o LibreOffice; `--no-pages` solo
  omite las imágenes de página.
- El índice de contenido se marca como `kind: "toc"`; una tabla de datos
  enorme se marca para filtrarla en vez de leerla; `.docpackignore` deja fuera
  archivos por patrón.
- Reemplazar un pack ya no deja la carpeta sin pack si algo falla a la mitad.
- Dos construcciones del mismo documento a la vez ya no comparten carpeta
  temporal (antes una podía borrarle la carpeta a la otra).

## 0.3.1

- Un pack armado sin páginas (por ejemplo, una vista previa rápida) se vuelve a
  armar cuando se piden páginas; antes se reutilizaba y las páginas nunca
  llegaban. El manifiesto guarda con qué opciones se armó.

## 0.3.0

- Páginas en Word: con Microsoft Word o LibreOffice instalados, cada sección
  lleva su página y el pack trae la imagen de cada página. Sin ellos, el pack
  lo dice y se cita por sección y línea.
- Tablas en PDF: se detectan por sus bordes y salen en CSV, con celdas
  combinadas, encabezados sombreados y tablas que siguen en la página
  siguiente.
- OCR de páginas escaneadas con el motor del sistema: OCR de Windows, Vision
  en macOS o Tesseract en Linux. El texto queda marcado como OCR.
- Imágenes de página también en macOS (Core Graphics) y Linux (`pdftoppm`).
- `--no-pages` ahora también omite la paginación de los Word.

## 0.2.0 · primera versión pública

- El repositorio se separa en `engine/` (motor y comando) y `plugins/claude-code/`
  (el plugin). El protocolo del hook de Claude Code vive solo en
  `engine/src/claude_code.rs`, y el hook ahora se llama `docpack hook claude-code`.
- Lugar reservado para un plugin de Codex en `plugins/codex/` y guía en
  `docs/integrar-otro-agente.md`.
- `scripts/build-claude-plugin.ps1` prueba, compila, copia el ejecutable al
  plugin y se detiene si la versión del motor y la del plugin no coinciden.
- Windows, Linux (x64 y ARM64) y macOS (Apple Silicon e Intel): un lanzador
  `bin/docpack` elige el ejecutable del sistema; los binarios los compila
  GitHub Actions y las pruebas corren en los tres sistemas.
- La conversión no cambia respecto a 0.1.3; los packs se regeneran una vez
  porque cambió la versión.

## 0.1.3 · correcciones tras la prueba con un diseño técnico real

Se comparó el pack contra una lectura directa y completa del documento (Word,
~88 000 tokens, 89 tablas). Fallas encontradas y corregidas, cada una con su
prueba en `engine/tests/docx.rs`:

| # | Falla | Ahora |
|---|---|---|
| 1 | Se perdían los símbolos insertados con «Insertar símbolo» (las flechas →) | 71 de 71 presentes; los de fuentes sin Unicode se avisan en `notes` |
| 2 | Un salto de línea dentro de una celda se volvía espacio | Se conserva el salto |
| 3 | Las listas perdían su nivel | Cada nivel lleva su sangría |
| 4 | La estimación de tokens se quedaba a la mitad en texto técnico | −1.4 % contra lo medido (antes −59 %) |
| 5 | El índice pesaba mucho con muchas tablas | 59 % más ligero; detalle en `tablas/index.json` y `figuras/index.json` |
| 6 | Un párrafo justo después de una lista quedaba pegado al último ítem | Línea en blanco después de la lista |
| 7 | Una imagen usada dos veces se guardaba dos veces | Una figura, dos referencias |
| 8 | Los logos de encabezado aparecían como figuras | Se cuentan en `notes`, no se exportan |
| 9 | Las líneas de un cuadro de texto se separaban | Quedan juntas |
| 10 | Un Excel incompleto daba un error técnico | «No es un libro de Excel completo: le falta …» |
| 11 | Una sección enorme se leía entera aunque solo hiciera falta un tramo | Se describe en partes de ~2 000 tokens con sus líneas |

Resultado: 6 393 de 6 393 celdas presentes.

## 0.1.2

- Títulos de Word por estilo integrado («heading N»), imágenes del documento en
  `figuras/` e índice compacto.

## 0.1.0 y 0.1.1

- Primeras versiones: PDF, DOCX, XLSX y PPTX a pack con índice, secciones,
  tablas CSV e imágenes de página; plugin con hook y skill.

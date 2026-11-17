# DocPack

[English](README.md) · **Español**

Cuando le pasas a Claude un PDF o un Word grande, lo lee completo aunque solo
quieras saber una cosa de la página 40. Eso cuesta tokens, llena el contexto y,
si el documento es muy largo, ni siquiera cabe en una sola lectura.

DocPack lo resuelve de una forma sencilla: convierte el documento **en tu
computadora** en una carpeta ordenada por partes. Claude abre primero un índice
pequeño, ve qué secciones y tablas hay y cuánto pesa cada una, y lee solo lo
que necesita para responder.

Además **conserva los colores con que el documento marca las cosas**: el perro
remarcado en rojo, la fila gris que no entra en la primera entrega, «solo
se prueba lo rosa». El pack trae la leyenda de colores del documento y el
color de cada celda y de cada palabra marcada, así que puedes preguntar por
color (`docpack find --color rosa`) en Word y en PDF.

Lo hice para mi propio trabajo con diseños técnicos largos y lo comparto tal
como lo uso. Viene integrado en [Sky Agent](https://ia.skyoflies.com),
funciona como plugin de Claude Code, se instala en Muse y, por MCP, en
OpenAI Codex, Kimi Code, Google Antigravity y clientes de DeepSeek. Más en
[ia.skyoflies.com/docpack.html](https://ia.skyoflies.com/docpack.html).

— Juan Antonio Sandoval Puente · [skyoflies.com](https://skyoflies.com)

<p align="center"><img src="docs/img/ahorro.svg" alt="Tokens para responder sobre 44 documentos reales: 27.8 millones leyéndolos completos (111 dólares con Claude Opus 5.5), 166 725 con DocPack en la primera pregunta (99.4 % menos) y 23 368 en las siguientes (99.9 % menos)" width="720"></p>

## Inicio rápido

Instala DocPack con su skill y su servidor MCP para tu agente (en Claude Code
es un plugin; abajo están todos):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent codex
```

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent codex
```

Cambia `codex` por `kimi`, `antigravity` o `deepseek`. O úsalo tú desde la
terminal con los documentos de [`examples/`](examples/):

```bash
docpack build clinic-inventory.xlsx      # el pack: índice, secciones, tablas, colores, fórmulas
docpack find --color rosa                # solo lo que el documento marca en rosa
docpack diff vaccination-plan-v1.0.docx vaccination-plan-v1.1.docx
```

## Documentos del día a día

Los documentos reales no son texto limpio: traen fórmulas, celdas rellenas
de espacios, títulos arriba de las tablas, filas en gris, texto en rojo y
versiones que cambian. DocPack conserva todo eso. Los ejemplos de
[`examples/`](examples/) son de ese tipo (un inventario en Excel y dos
versiones de un plan en Word). Abajo va un resumen de la salida real, que es JSON.

**«Prueba solo lo rosa».** El plan marca en rosa lo que entra en la primera
entrega. `docpack find --color rosa` devuelve justo esas filas, con su
tabla, sección y página:

```text
2. Schedule · T01 fila 2   Dog | Parvovirus | 6 weeks | 1 ml
2. Schedule · T01 fila 4   Cat | Triple feline | 8 weeks | 0.5 ml
```

**Qué cambió entre versiones.** `docpack diff` compara secciones y filas de
tablas, no bytes:

```text
Tabla «2. Schedule»   − Dog | Rabies | 12 weeks | 1 ml
                      + Dog | Rabies | 12 weeks | 1.5 ml
                      + Rabbit | Myxomatosis | 10 weeks | 0.5 ml
Sección «1. Scope»    + … From version 1.1 it also covers rabbits. …
Sección «3. Records»  + … Rabbits get a card of their own.
```

**Excel como se usa de verdad.** En el inventario:

| En el libro | En el pack |
|---|---|
| Celdas rellenas con espacios (exportación de ancho fijo) | se quitan; el manifiesto dice cuántos. En un libro real de 10 hojas: 2.18 MB → 231 KB de CSV |
| `=D4*E4`, `=SUMIF(Inventory!A4:A10,"VAC*",…)` | el CSV lleva el valor; `Tnn.formulas.json`, la fórmula de cada celda |
| Título combinado arriba de la tabla | nombra la tabla; el encabezado es la fila real |
| Fila en rojo, fila en gris, celda amarilla, una palabra en rojo | `Tnn.colores.json` y la leyenda del manifiesto |

## Métricas

### Tokens: 44 documentos reales

Medido con los mismos 44 documentos de trabajo de las demás métricas: 27
guías técnicas en PDF (de 2 a 3 220 páginas), 7 diseños en Word y 10 libros
de Excel. Para cada documento:

- **Sin DocPack** es leerlo completo: todo su texto y sus tablas. Es el mínimo;
  leer un PDF original suma además la imagen de cada página.
- **Con DocPack, la primera pregunta** es su índice más la parte que hace falta
  (la mediana de sus partes, secciones cortas y tablas).
- **Cada pregunta siguiente** lee solo esa parte: el índice ya está leído.

Los tokens son la estimación que trae cada pack (dentro de un 1.4 % de lo
medido en un diseño en Word); los dólares, al precio de entrada de Claude
Opus 5.5, 4 dólares por millón de tokens.

| 44 documentos | Sin DocPack | Con DocPack | Ahorro |
|---|---:|---:|---:|
| Primera pregunta en cada documento | 27.8 M tokens · $111.08 | 166 725 · $0.67 | **99.4 %** |
| Cada pregunta siguiente | 27.8 M · $111.08 | 23 368 · $0.09 | **99.9 %** |
| Diez preguntas en cada documento | 277.7 M · $1 110.84 | 377 037 · $1.51 | **99.9 %** |
| Mediana por documento (1.ª pregunta · siguientes) | | | **94.9 % · 99.6 %** |
| Documentos que no caben en una conversación (más de un millón de tokens) | 6 | 0 | |

| Por tipo | Documentos | Leerlos completos | 1.ª pregunta | Ahorro | Siguientes | Ahorro |
|---|---:|---:|---:|---:|---:|---:|
| PDF | 27 | 11.1 M | 119 576 | 98.9 % | 17 467 | 99.8 % |
| Word | 7 | 311 400 | 29 149 | 90.6 % | 2 144 | 99.3 % |
| Excel | 10 | 16.3 M | 18 000 | 99.9 % | 3 757 | 99.98 % |

Las diez preguntas suponen que sin DocPack el documento se vuelve a leer en
cada una; si sigue en el contexto no se relee, pero ocupa esos tokens en toda
la conversación. El ahorro es menor en los documentos chicos: en uno de dos
páginas, el índice pesa la mitad del documento.

### Velocidad

| | Resultado |
|---|---:|
| Convertir el diseño de 88 000 tokens | 0.14 a 0.3 s |
| Seis documentos de prueba (Word, Excel, PDF) | 139 a 295 ms cada uno |
| Paginar un Word de 108 páginas con Word, con sus imágenes de página | 20 a 30 s, solo la primera vez |
| OCR de una página escaneada | ~0.5 s |
| Volver a abrir un documento sin cambios | instantáneo: se reusa el pack por su huella SHA-256 |
| Tokens gastados al convertir | **0**, todo pasa en tu máquina |

### Fidelidad

| Qué se revisó | Resultado |
|---|---:|
| Celdas de tablas presentes en el pack | **6 393 de 6 393** |
| Símbolos insertados con «Insertar símbolo» de Word (flechas →) | **71 de 71** |
| Error de la estimación de tokens del índice | **−1.4 %** (86 904 estimados, ~88 120 medidos) |
| Peso del índice tras optimizarlo | 19 105 → 7 829 caracteres (**−59 %**) |
| Imagen repetida en el documento | se guarda una vez y se referencia dos |
| Páginas del Word (paginado por Word) | **33 de 33** secciones en su página correcta |
| Tablas del mismo diseño exportado a PDF | **6 258 de 6 323** celdas iguales a las del Word (99 %) |
| Página escaneada (solo imagen, sin texto) | texto completo con el OCR del sistema |
| Guía técnica en PDF de 2 409 páginas con tablas sin bordes | de **0** a **2 402** tablas en CSV: el **98 %** de las que la guía numera |
| Tablas revisadas a mano en guías PDF y diseños Word reales (sin bordes, partidas en 2 y 3 páginas) | **9 de 9** iguales |
| Colores: texto marcado en rojo en un diseño técnico real | **670** pedazos en rojo, los mismos en el Word y en su PDF |
| Colores: filas en gris (campos fuera de la primera entrega) en otro diseño | **382** celdas, con sus filas exactas |

<p align="center"><img src="docs/img/tablas.svg" alt="Tablas leídas en guías PDF reales: en una guía de 2 409 páginas sin bordes, de 0 a 2 402 tablas, el 98 % de las que numera; en un manual de 3 220 páginas con tablas partidas, del 80 % al 95 % en un CSV propio; 9 de 9 tablas revisadas a mano iguales" width="720"></p>

### En 44 documentos reales (0.4.1)

Un documento dice poco; estas cifras salen de convertir con la 0.4.0 y con la
0.4.1 los mismos 44 documentos de trabajo: 27 guías técnicas en PDF (de 2 a
3 220 páginas), 7 diseños en Word y 10 libros de Excel. Cada métrica la cuenta
un script sobre los dos packs, y el contenido se comparó palabra por palabra.

<p align="center"><img src="docs/img/corpus.svg" alt="0.4.0 contra 0.4.1 en 44 documentos reales: tablas numeradas con CSV propio de 88 % a 97 %; tablas que se comían a la siguiente de 112 a 2; palabras con letras desordenadas de 1 448 a 0; encabezados y pies repetidos de 1.72 millones a 0.08 millones de caracteres; tokens de texto de 7.7 millones a 5.2 millones" width="720"></p>

| | 0.4.0 | 0.4.1 |
|---|---:|---:|
| Tablas que las guías numeran («Table 16:») con su CSV propio | 5 286 de 6 003 (88 %) | **5 826 de 6 003 (97 %)** |
| Tablas que se quedaban con el título y las filas de la siguiente | 112 | **2** |
| Palabras con letras desordenadas («sufb ield» por «subfield») | 1 448 | **0** |
| Caracteres de encabezados y pies repetidos en el texto | 1 718 779 | **75 372** (−96 %) |
| Tokens de texto de las 27 guías PDF | 7.7 millones | **5.2 millones** (−32 %) |
| Subtítulos de los marcadores del PDF dentro de las secciones | 0 | **4 557** |
| Páginas apaisadas leídas letra por letra (líneas de una sola letra, en una guía) | 191 694 | **3 994** |
| Tablas numeradas sin CSV propio que el pack nombra con su página | ninguna | **todas** |
| Word y Excel | | idénticos |
| Tiempo de conversión de los 44 | 249 s | 255 s |

Menos tokens no es menos contenido: lo que salió es lo que se repetía en cada
página (el aviso de copyright, el nombre de la guía, el número de página) y el
texto que antes salía roto. Lo que sí se leía antes se sigue leyendo.

### El paquete

| | |
|---|---:|
| Ejecutable | ~3 MB, sin dependencias |
| Necesita internet | no |
| Necesita Python o Node | no |
| Necesita Office | no; si hay Word o LibreOffice, los Word salen con número de página |

Los números cambian con cada documento, pero la idea se mantiene: **pagas por
lo que lees, no por el documento entero.**

## Instalación

DocPack corre en tu máquina y lee tus documentos igual en todos los agentes:
el mismo ejecutable, el mismo pack y las mismas reglas de lectura.

| | Agente | Cómo se conecta | Instalación |
|:-:|---|---|---|
| <img src="docs/assets/agentes/claude.svg" width="32" height="32" alt="Claude"> | **Claude Code** | Plugin (hook + skill) | [Claude Code](#claude-code) |
| <img src="docs/assets/agentes/muse.svg" width="32" height="32" alt="Muse"> | **Muse** (Meta) | Instalación local + skill | [Muse](#muse) |
| <img src="docs/assets/agentes/codex.svg" width="32" height="32" alt="OpenAI Codex"> | **OpenAI Codex** | Servidor MCP + skill | [Codex](#openai-codex) |
| <img src="docs/assets/agentes/kimi.svg" width="32" height="32" alt="Kimi"> | **Kimi Code** | Servidor MCP + skill | [Kimi](#kimi-code) |
| <img src="docs/assets/agentes/antigravity.svg" width="32" height="32" alt="Google Antigravity"> | **Google Antigravity** | Servidor MCP + skill | [Antigravity](#google-antigravity) |
| <img src="docs/assets/agentes/deepseek.svg" width="32" height="32" alt="DeepSeek"> | **DeepSeek** (DeepSeek-TUI, OpenCode, Cline…) | Servidor MCP + skill | [DeepSeek](#deepseek) |

### Claude Code

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/claude-oscuro.png">
  <img src="docs/img/claude.png" alt="Claude" height="48">
</picture>

Dentro de Claude Code:

```
/plugin marketplace add xskyofx-lang/docpack
/plugin install docpack@docpack
/reload-plugins
```

O desde la terminal:

```bash
claude plugin marketplace add xskyofx-lang/docpack
```

```bash
claude plugin install docpack@docpack
```

Eso es todo. No hay que instalar Python, ni Rust, ni nada más: el plugin trae
el ejecutable de DocPack ya compilado.

**Sistemas:** Windows (x64), Linux (x64 y ARM64) y macOS (Apple Silicon e
Intel). El plugin trae el ejecutable de cada uno y elige solo el que toca.

Algunas funciones usan lo que tu sistema ya trae. Si falta algo, el pack se
crea igual y `manifest.json` dice qué no se pudo hacer y por qué:

| Función | Windows | macOS | Linux |
|---|---|---|---|
| Imágenes de página | incluido en Windows | incluido en macOS | `pdftoppm` (paquete `poppler-utils`) |
| OCR de páginas escaneadas | OCR de Windows, en los idiomas de tu perfil | Vision de macOS, en tus idiomas | `tesseract` con sus idiomas (por ejemplo `tesseract-ocr-spa`) |
| Números de página en Word | Microsoft Word o LibreOffice | LibreOffice | LibreOffice |

### Muse

<p><img src="docs/img/muse.jpg" alt="Muse" height="160"></p>

Muse no instala plugins, pero su agente tiene su propia máquina Linux y puede
instalar programas. Mándale este mensaje:

```
Instala DocPack en tu máquina: descarga y ejecuta
https://raw.githubusercontent.com/xskyofx-lang/docpack/main/plugins/muse/install.sh
y luego guarda como skill el archivo
https://raw.githubusercontent.com/xskyofx-lang/docpack/main/plugins/muse/SKILL.md
para que de ahora en adelante leas mis documentos PDF, Word, Excel y
PowerPoint a través de su pack, no del archivo original.
```

- `install.sh` detecta la plataforma (Linux x64 o ARM64, macOS), descarga el
  mismo ejecutable que trae el plugin de Claude Code a `~/.local/bin/docpack`
  y lo verifica con `docpack --version`. No necesita Rust, Python ni compilar.
- `SKILL.md` es la regla de lectura por partes, la misma que sigue Claude.
- **Actualizaciones:** una vez por semana Muse corre `check-update.sh`, que
  compara su versión con la publicada en este repositorio. Si hay una nueva,
  te dice cuál es y qué mejora (del CHANGELOG) y te pregunta si la instala;
  nunca actualiza sin tu permiso.

Probado en la máquina de Muse el 14 de noviembre de 2026 con un Word real.
Los detalles del adaptador están en
[plugins/muse/README.md](plugins/muse/README.md).

### OpenAI Codex

<img src="docs/assets/agentes/codex.svg" width="48" height="48" alt="OpenAI Codex">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent codex
```

**macOS y Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent codex
```

Registra el servidor con `codex mcp add` (queda en `~/.codex/config.toml`) y deja la skill en `~/.agents/skills/docpack/`. Antes de editar una configuración deja una copia `*.antes-de-docpack`.
Reinicia el agente al terminar. Detalles en [plugins/codex/README.md](plugins/codex/README.md).

### Kimi Code

<img src="docs/assets/agentes/kimi.svg" width="48" height="48" alt="Kimi Code">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent kimi
```

**macOS y Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent kimi
```

Registra el servidor en `~/.kimi-code/mcp.json` y deja la skill en `~/.agents/skills/docpack/`. En Kimi, `/mcp` muestra si está conectado. Antes de editar una configuración deja una copia `*.antes-de-docpack`.
Reinicia el agente al terminar. Detalles en [plugins/kimi/README.md](plugins/kimi/README.md).

### Google Antigravity

<img src="docs/assets/agentes/antigravity.svg" width="48" height="48" alt="Google Antigravity">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent antigravity
```

**macOS y Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent antigravity
```

Registra el servidor en `~/.gemini/config/mcp_config.json` (lo comparten la app, el IDE y la CLI) y deja la skill en `~/.gemini/config/skills/docpack/`. Antes de editar una configuración deja una copia `*.antes-de-docpack`.
Reinicia el agente al terminar. Detalles en [plugins/antigravity/README.md](plugins/antigravity/README.md).

### DeepSeek

<img src="docs/assets/agentes/deepseek.svg" width="48" height="48" alt="DeepSeek">

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent deepseek
```

**macOS y Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent deepseek
```

DeepSeek no tiene un archivo de configuración único: el instalador deja la skill en `~/.agents/skills/docpack/` y te imprime la entrada MCP exacta para pegarla en tu cliente (DeepSeek-TUI, OpenCode, Cline, Roo Code…). Antes de editar una configuración deja una copia `*.antes-de-docpack`.
Reinicia el agente al terminar. Detalles en [plugins/deepseek/README.md](plugins/deepseek/README.md).

### Solo el comando `docpack`

Para usarlo desde la terminal o conectarlo a otro cliente MCP
(`docpack mcp` es un servidor MCP por stdio):

```powershell
irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1 | iex
```

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash
```

Instala el ejecutable en `%LOCALAPPDATA%\Programs\DocPack` (y lo agrega a tu
PATH de usuario; `-NoPath` no toca el PATH) o en `~/.local/bin`.

O descarga el ejecutable de tu sistema del
[último release](https://github.com/xskyofx-lang/docpack/releases/latest)
(con sus sumas SHA-256), o compílalo con Rust:

```bash
cargo install --git https://github.com/xskyofx-lang/docpack docpack --locked
```

## Cómo usarlo sin gastar tokens de más

En la práctica no tienes que hacer nada especial: pídele a Claude lo que
necesites del documento y él se encarga. Aun así hay cosas que marcan la
diferencia.

**Dale la ruta, no el adjunto.** Si arrastras el PDF al chat, el archivo le
llega entero a Claude antes de que cualquier plugin pueda hacer algo. Mejor
déjalo en la carpeta del proyecto y menciónalo:

> «En `docs/diseno.docx`, ¿qué validaciones tiene el campo de fecha?»

**Pregunta por algo concreto.** «¿Qué dice la sección de errores?» hace que lea
una sección. «Resúmeme todo el documento» lo obliga a leerlo todo, con o sin
DocPack.

**Sigue en la misma conversación.** La primera pregunta paga el índice; las
siguientes ya lo tienen y solo leen la parte nueva.

**El original, solo si de verdad lo necesitas.** Claude no abre el documento
original a menos que tú se lo pidas, de forma explícita y con páginas:

> «Mira la página 12 del PDF original, quiero ver el diagrama.»

Eso sí lo deja pasar, y solo esas páginas.

**Si cambias el documento, no hagas nada.** DocPack se da cuenta por la huella
del archivo y vuelve a convertirlo la siguiente vez que Claude lo lea.

## Cómo funciona por dentro

Todo pasa en tu máquina. DocPack no se conecta a internet, no manda el
documento a ningún lado y no usa servicios externos: lee la estructura que el
propio archivo ya trae y, cuando hace falta, usa los motores que tu sistema
tiene instalados (OCR, Word o LibreOffice para paginar).

1. Claude intenta leer `diseno.docx`.
2. Antes de que esa lectura ocurra, el plugin (un *hook* de Claude Code)
   ejecuta DocPack. Si ya hay un pack de ese archivo y su huella coincide, lo
   reusa; si no, lo crea.
3. En lugar del documento, Claude recibe un aviso corto: «este documento está
   en `docpack/diseno.docx/`, empieza por `index.json`».
4. Claude abre el índice y después solo las secciones o tablas que necesita.

El documento original nunca se modifica. El pack queda junto a tu proyecto:

```
tu-proyecto/
  diseno.docx
  docpack/
    index.json        catálogo: cada pack con título, versión y fecha del documento
    diseno.docx/
      manifest.json   de dónde salió, huella SHA-256, título/versión/fecha, avisos
      index.json      secciones, líneas y tokens estimados → se lee primero
      secciones/      01-objetivo.md, 02-alcance.md …
      tablas/         T01.csv … e index.json (título, encabezado y páginas de cada tabla)
      figuras/        F01.png …  (imágenes del Word) e index.json
      paginas/        p001.png … (cada página como imagen, o solo las que hay que revisar)
```

Cómo se parte cada formato:

| Formato | Secciones | Tablas | Además |
|---|---|---|---|
| Word (DOCX) | una por cada título, según los estilos del documento | CSV, con el color de cada celda aparte | página de cada sección e imagen de cada página (con Word o LibreOffice), imágenes en `figuras/`, listas con su nivel, encabezados y pies, texto en color |
| PDF | según los marcadores del PDF; si no tiene, una por página. Los marcadores más finos (un campo, un subcampo) quedan como subtítulos `###` dentro de la sección | CSV, con o sin bordes, aunque sigan en las páginas siguientes, con el color de cada celda aparte | OCR en las páginas escaneadas, imagen de cada página, índice de contenido marcado, páginas apaisadas enderezadas, sin el encabezado y el pie que se repiten |
| Excel (XLSX, XLS, XLSB, ODS) | una por hoja | un CSV por hoja, con su encabezado aunque la hoja traiga un título arriba | fórmulas de cada celda aparte, sin los espacios de relleno, colores de celda y de texto (XLSX) |
| PowerPoint (PPTX) | una por diapositiva, en el orden de la presentación | CSV | número de diapositiva |

Una sección muy larga viene dividida en partes de unos 2 000 tokens con sus
números de línea, para que Claude lea solo el tramo que importa. En un
documento muy largo, el índice se queda con lo que sirve para elegir la
sección (título, páginas, tokens, cuántas tablas y partes tiene) y el detalle
de cada sección va en `secciones/index.json`: en una guía de 2 409 páginas el
índice pasa de 104 KB a 32 KB. Si la sección
tiene subtítulos, las partes empiezan en ellos y cada una dice cuál es: «PDS
0158 (Business Activity)» en lugar de una frase suelta.

Cómo se sacan las partes que el archivo no trae escritas:

- **Páginas de un Word.** Un DOCX no guarda páginas: existen solo cuando un
  programa acomoda el texto. DocPack le pide a Word (o a LibreOffice), en
  segundo plano y sobre una copia, que exporte el documento a PDF con un
  marcador por título, y cada sección toma la página de su título. Si Word ya
  estaba abierto con documentos tuyos, no lo cierra.
- **Tablas de un PDF.** Se arman con los bordes que dibuja el PDF y la
  posición de cada letra: celdas combinadas, encabezados sombreados, palabras
  que la celda partió en dos renglones y tablas abiertas por los lados. Una
  tabla **sin bordes** se lee por sus columnas: los carriles en blanco que
  bajan por todas sus líneas (un párrafo los cierra enseguida), las reglas
  entre filas si las hay, y celdas de varias líneas. Una tabla que sigue en la
  página siguiente se une en un solo CSV aunque cambie el encabezado de la
  página o repita su título. Una tabla termina donde empieza un subtítulo o el
  título de la siguiente («Table 17:»), así que nunca se queda con las filas de
  otra. Cada tabla lleva su título (el pie o la línea de encima), su encabezado
  y sus páginas. Su texto va al CSV y no se repite en la sección.
- **El propio documento revisa las tablas.** Si el documento numera sus
  tablas, DocPack cuenta cada «Table N:» contra los CSV: la que quedó como
  texto se nombra en `manifest.json` con su página, y la que lleva el título de
  otra dentro se marca con `check` en `tablas/index.json`. Esas páginas llevan
  su imagen para verlas antes de citar una celda.
- **Encabezados y pies.** Lo que se repite en el margen de cada página (el
  copyright, el nombre de la guía, el número de página) sale del texto, y la
  línea del encabezado que cambia (el tema que empieza) se queda.
- **Páginas apaisadas.** Un reporte girado dentro de una página vertical se
  endereza según hacia dónde corren sus letras, y se lee de izquierda a derecha
  con sus columnas. Las letras que corren en otra dirección que el resto de la
  página (un pie de página de lado, una etiqueta en el margen) se leen aparte,
  en su propia dirección.
- **Listas dentro de una celda.** Una viñeta o un guion pegado a su texto es
  sangría, no una columna: una tabla de criterios con una lista de códigos en
  una celda sigue siendo una tabla de dos columnas.
- **Colores.** Cuando un documento marca cosas con color (el perro remarcado
  en rojo, la fila gris que no entra en la primera entrega, «solo se prueba
  lo rosa»), el pack lo conserva. `manifest.json` trae la leyenda: cada color
  con su nombre (rojo, rosa, morado, gris…), su valor, si es de texto, de
  fondo o resaltado, cuántas veces aparece y ejemplos. `tablas/Tnn.colores.json`
  da el fondo de cada celda y los pedazos de su texto que van en otro color;
  el CSV no cambia. En las secciones, el texto marcado va como
  `<span style="color:#FF0000">`. El color de cada palabra sale del documento:
  en Word, de la letra, de su estilo o del resaltado; en PDF, del color con
  que se pinta cada letra y del relleno detrás de cada celda. El color del
  cuerpo del texto (el que no marca nada) también sale del documento: de sus
  estilos en Word y del color que más se usa en PDF. En PDF, una línea toda de
  un color es el estilo de un título y no se marca.
- **Páginas escaneadas.** Una página que es sobre todo una imagen pasa por el
  OCR del sistema. Su texto queda marcado como OCR en la sección
  (`<!-- página 3 · texto por OCR -->`) y `manifest.json` dice qué motor lo
  leyó.

Lo que hay que tener en cuenta (y el pack lo avisa cuando aplica):

- Las tablas sin bordes se leen por la alineación de su texto: el pack dice en
  qué páginas. Si el documento numera sus tablas, el pack también nombra las
  que no quedaron en su CSV. Si una página parece tabla pero no se pudo leer como tal, queda
  como texto y su imagen va en `paginas/` aunque el pack no lleve imágenes,
  para verla antes de citar una celda.
- El OCR puede equivocarse en letras o números; para un dato crítico, pide ver
  la imagen de la página.
- En Word, el sombreado que pone el estilo de la tabla (filas alternas) no se
  lee como marca; el que se le pone a una celda sí. En PDF, los colores de
  tinta plana (Separation, DeviceN) no tienen un valor RGB que dar y no se
  marcan.
- Sin Word ni LibreOffice, las secciones de un Word van sin número de página y
  se citan por sección y línea.

## Usarlo sin Claude (línea de comandos)

El plugin pone el comando `docpack` en el PATH de Claude, pero también puedes
usarlo tú directamente:

```bash
docpack build diseno.pdf --project .
```

Opciones: `--force` reconstruye aunque el pack esté al día y `--no-pages` no
genera la imagen de cada página (los Word se paginan igual si hay con qué). La
salida es un JSON con la ruta del pack y sus números.

| Comando | Para qué |
|---|---|
| `docpack text diseno.pdf --pages 4-5` | El texto de esas páginas, desde el pack, sin abrir el original |
| `docpack find "ciclo de compensación"` | Buscar en todos los packs del proyecto: pack, sección o tabla, parte, página y línea. Primero lo escrito tal cual y los títulos de tabla y subtítulos; el índice de contenido al final; una línea repetida en cada página es un solo resultado. Filtros: `--pack`, `--in tablas` o `--in texto` |
| `docpack find --color rosa` | Lo que el documento marca en ese color: el nombre en español o en inglés (`rosa`, `pink`) o el hex (`--color FF00FF`); las celdas y los pedazos de texto, con o sin texto a buscar |
| `docpack diff diseno_v1.3.docx diseno_v1.4.docx` | Qué cambió entre dos versiones: secciones y filas de tablas |
| `docpack doctor` | Packs a medias o desactualizados, archivos ajenos en `docpack/` y documentos sin pack |
| `docpack catalog` | Rehace `docpack/index.json` (se actualiza solo con cada pack) |
| `docpack mcp` | Servidor MCP por stdio con `docpack_build`, `docpack_text`, `docpack_find`, `docpack_diff`, `docpack_catalog` y `docpack_doctor` |

Todos aceptan `--project <carpeta>`. Un archivo `.docpackignore` en el proyecto
(un patrón por línea, por ejemplo `exportacion_*.xls`) deja fuera los
archivos de datos que no tiene caso convertir.

## Actualizaciones

Cada versión del plugin lleva su número en `plugin.json` (por ejemplo `0.3.0`).
Claude Code se queda con la versión que instalaste hasta que ese número cambia;
por eso, cuando corrijo algo, publico una versión nueva con otro número.

Para recibirla:

```bash
claude plugin marketplace update docpack
```

```bash
claude plugin update docpack@docpack
```

y reinicia Claude Code (o usa `/reload-plugins`). Si prefieres no pensar en
eso, activa la actualización automática en `/plugin` → **Marketplaces** →
docpack → **Enable auto-update**: Claude Code revisará por su cuenta y te
avisará cuando haya algo nuevo.

Tus packs no se quedan viejos. Cada uno guarda la versión de DocPack que lo
creó, y cuando esa versión cambia, el pack se vuelve a generar solo la próxima
vez que se lee. Lo que cambió en cada versión está en
[CHANGELOG.md](CHANGELOG.md).

## Cómo está organizado el repositorio

```
engine/                 el motor: biblioteca y comando docpack (Rust)
  src/lib.rs            convierte y arma el pack, sin saber nada de ningún agente
  src/claude_code.rs    el único archivo que conoce el protocolo de Claude Code
  src/mcp.rs            el servidor MCP (docpack mcp) para los demás agentes
  vendor/pdf-extract/   pdf-extract 0.12.1 (MIT) con la lectura de anchos de
                        fuente corregida: ver engine/Cargo.toml
plugins/
  claude-code/          el plugin de Claude Code: hook, skill, bin/ (lanzador y
                        ejecutable de Windows) y native/ (Linux y macOS)
  muse/                 instalación para Muse: install.sh, SKILL.md y
                        check-update.sh (usa los ejecutables de native/)
  codex/ kimi/          skill, .mcp.json y README de cada agente que usa el
  antigravity/          servidor MCP
  deepseek/
install/                instaladores para Windows (install.ps1) y macOS/Linux
                        (install.sh), con --agent para registrar la skill y MCP
docs/                   formato del pack y cómo conectar otro agente
  assets/agentes/       logotipos de cada agente
scripts/                compilar y empaquetar en local
.github/workflows/      pruebas y binarios para los tres sistemas
```

El motor no depende de Claude. Un agente con MCP lo usa a través de
`docpack mcp`; uno sin MCP solo necesita su propio adaptador y su carpeta en
`plugins/`. Los detalles
están en [docs/integrar-otro-agente.md](docs/integrar-otro-agente.md).

## Compilar desde el código

Hace falta [Rust](https://rustup.rs) 1.80 o más nuevo.

```bash
cargo build --release --manifest-path engine/Cargo.toml
```

Para probar el plugin en tu Windows con lo que acabas de compilar (pruebas,
compilación, copia del ejecutable y verificación de que las versiones
coinciden):

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-claude-plugin.ps1
```

Los ejecutables que se publican no salen de la máquina de nadie: los compila
GitHub Actions en Windows, Linux y macOS (workflow **Binarios**), comprueba que
cada uno responde con la versión del plugin y los deja en
`plugins/claude-code/` en un commit. Las pruebas corren en los tres sistemas
con cada cambio (workflow **Pruebas**).

## Licencia

MIT. Úsalo, cámbialo y compártelo; solo conserva el aviso de autoría.

Los logotipos de `docs/assets/agentes/` son marcas de sus dueños y aparecen
solo para indicar con qué agente funciona DocPack. Vienen de
[Simple Icons](https://simpleicons.org) (CC0) y, el de Antigravity, del sitio
oficial de Google Antigravity.

Hecho por Juan Antonio Sandoval Puente · [skyoflies.com](https://skyoflies.com)

# Cambios

## 0.2.0 · primera versión pública

- El repositorio se separa en `engine/` (motor y comando) y `plugins/claude-code/`
  (el plugin). El protocolo del hook de Claude Code vive solo en
  `engine/src/claude_code.rs`, y el hook ahora se llama `docpack hook claude-code`.
- Lugar reservado para un plugin de Codex en `plugins/codex/` y guía en
  `docs/integrar-otro-agente.md`.
- `scripts/build-claude-plugin.ps1` prueba, compila, copia el ejecutable al
  plugin y se detiene si la versión del motor y la del plugin no coinciden.
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

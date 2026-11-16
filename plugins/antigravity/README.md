# <img src="../../docs/assets/agentes/antigravity.svg" width="28" height="28" alt="" align="top"> DocPack para Google Antigravity

DocPack convierte PDF, Word, Excel y PowerPoint en un pack local (índice,
secciones, tablas CSV, colores e imágenes de página) para que el agente lea
solo las partes que necesita, no el documento entero. Funciona con
la app, el IDE y la CLI de Antigravity, a través de un servidor MCP local (`docpack mcp`) y una skill.

## Instalación

**Windows** (PowerShell):

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent antigravity
```

**macOS y Linux**:

```bash
curl -fsSL https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.sh | bash -s -- --agent antigravity
```

El instalador:

1. Descarga el binario de tu plataforma (Windows x64, macOS Apple Silicon e
   Intel, Linux x64 y ARM64) en `%LOCALAPPDATA%\Programs\DocPack\docpack.exe`
   o `~/.local/bin/docpack`. No necesita Rust, Python ni compilar nada.
2. Instala la skill en `~/.gemini/config/skills/docpack/SKILL.md`.
3. Registra el servidor MCP en `~/.gemini/config/mcp_config.json` bajo `mcpServers` (lo comparten la app, el IDE y la CLI). Antes de editar un archivo de
   configuración deja una copia `*.antes-de-docpack` junto a él.

Reinicia Google Antigravity al terminar. Para un solo proyecto, la skill también funciona en `<proyecto>/.agents/skills/docpack/` y el servidor en `<proyecto>/.agents/mcp_config.json`.

Para no tocar el PATH de usuario en Windows, agrega `-NoPath`.

## Configuración manual

Si prefieres hacerlo a mano, la entrada MCP es la de [`.mcp.json`](.mcp.json)
(cambia `docpack` por la ruta completa del binario si no está en tu PATH):

```json
{ "mcpServers": { "docpack": { "command": "docpack", "args": ["mcp"] } } }
```

y la skill es [`SKILL.md`](SKILL.md).

## Herramientas MCP

| Herramienta | Qué hace |
|---|---|
| `docpack_build` | Crea o reutiliza el pack de un documento (`file`, `project`, `pages`, `force`) |
| `docpack_text` | Texto de páginas concretas (`file`, `pages` como `"4-5"`) |
| `docpack_find` | Busca en todos los packs del proyecto, también por color (`query`, `color`, `in`, `pack`) |
| `docpack_diff` | Compara dos versiones de un documento (`old`, `new`) |
| `docpack_catalog` | Escribe `docpack/index.json` con todos los packs |
| `docpack_doctor` | Revisa packs viejos, incompletos o de documentos que cambiaron |

Una ruta relativa en `file` se toma desde `project`. Sin `project`, desde la
carpeta en la que el agente arrancó el servidor.

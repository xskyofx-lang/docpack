# Conectar DocPack a otro agente

DocPack está partido en dos a propósito. El **motor** (`engine/`) convierte
documentos y arma el pack; no sabe nada de Claude ni de ningún otro agente. Cada
**plugin** (`plugins/<agente>/`) es la capa delgada que le explica a un agente
concreto cómo usarlo.

Para sacar el plugin de Codex, o el de cualquier otro, no hay que tocar el
motor.

## Lo que el motor ofrece

**Comando:**

```bash
docpack build <archivo> --project <carpeta> [--no-pages] [--force]
```

- Crea o reusa `<carpeta>/docpack/<nombre del archivo>/`. Si el archivo está
  fuera de la carpeta del proyecto, el pack queda junto al archivo.
- Imprime un JSON con `pack`, `index`, `status` (`built` o `up_to_date`),
  `sections`, `tables`, `page_images` y `tokens_estimate`.
- Código de salida 0 si salió bien, 1 si el documento no se pudo convertir, 2
  si los argumentos están mal.

```bash
docpack text <archivo> --pages 4-5 [--project <carpeta>]
docpack find "<texto>" [--project <carpeta>] [--limit N]
docpack diff <pack o documento> <pack o documento> [--project <carpeta>]
docpack doctor [--project <carpeta>]
docpack catalog [--project <carpeta>]
docpack --version
```

`text` imprime texto; `find`, `diff` y `doctor` imprimen JSON.

**Biblioteca (Rust):** `docpack::build`, `docpack::extract`, `docpack::supported`,
`docpack::tokens_estimate` y `docpack::library` (`find`, `diff`, `doctor`,
`page_text`, `write_catalog`, `ignored`), si el adaptador se escribe en Rust
dentro del mismo ejecutable.

## Lo que necesita un plugin nuevo

1. **Una forma de enterarse** de que el agente va a leer un documento. En Claude
   Code es un hook `PreToolUse` sobre `Read`. Si el agente no tiene hooks,
   basta con las instrucciones del punto 2: el agente ejecuta `docpack build`
   él mismo.
2. **Instrucciones para el agente**: leer `index.json` primero, abrir solo las
   secciones y tablas necesarias, y el original solo si la persona lo pide.
   La versión de Claude está en `plugins/claude-code/skills/docpack/SKILL.md` y
   sirve de base.
3. **El ejecutable**, compilado con `cargo build --release --manifest-path
   engine/Cargo.toml`.

Si el agente habla un protocolo propio (como el JSON del hook de Claude Code),
ese protocolo va en un archivo propio junto a `engine/src/claude_code.rs`, por
ejemplo `engine/src/codex.rs`, con su subcomando `docpack hook codex`. Así cada
protocolo queda aislado y el motor no cambia.

## Formato del pack

```
docpack/index.json  packs[] {pack, document, kind, pages, title?, version?, date?, sections, tables, tokens_estimate, sha256, docpack_version}
docpack/<archivo>/
  manifest.json   source {file, name, sha256, bytes, kind, pages}, document {title?, version?, date?}, docpack_version, options, stats, notes
  index.json      sections[] {id, title, kind?, level, path?, page_start?, page_end?, file, lines, tokens_estimate, parts?, outline?, tables?, figures?}, page_images {count, file, only?}
  secciones/      NN-<título>.md
  tablas/         Tnn.csv, index.json [{id, title, section, page, pages?, rows, columns, header, file, tokens_estimate, read?}]
  figuras/        Fnn.<ext>, index.json
  paginas/        pNNN.png  (cada página como imagen, de un PDF o de un Word)
```

El pack se considera al día cuando `manifest.json` tiene la misma huella SHA-256
del documento y la misma `docpack_version` del ejecutable. DocPack no
sobrescribe una carpeta que no tenga un `manifest.json` suyo.

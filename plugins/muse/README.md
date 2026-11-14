# DocPack para Muse — adaptador `plugins/muse/`

DocPack corre **local en la máquina de cada Muse**, no en un servidor.
Cada usuario de Muse tiene su propio agente con su propia VM Linux, así
que cualquiera puede instalarlo con un prompt, sin plugins y sin que Meta
revise nada.

## Cómo se instala (lo que el usuario le manda a su Muse)

```
Instala DocPack en tu máquina: descarga y ejecuta
https://raw.githubusercontent.com/xskyofx-lang/docpack/main/plugins/muse/install.sh
y luego guarda como skill el archivo
https://raw.githubusercontent.com/xskyofx-lang/docpack/main/plugins/muse/SKILL.md
para que de ahora en adelante leas mis documentos PDF, Word, Excel y
PowerPoint a través de su pack, no del archivo original.
```

## Archivos

- `install.sh` — detecta la plataforma (linux-x64, linux-arm64,
  macos-arm64, macos-x64), descarga el binario precompilado que ya trae el
  repo en `plugins/claude-code/native/<plataforma>/docpack`, lo deja en
  `~/.local/bin/docpack` y verifica con `docpack --version`. No necesita
  Rust, Python ni compilar nada. También descarga `check-update.sh`.
- `check-update.sh` — compara la versión instalada con la publicada en
  `main` (`plugins/claude-code/.claude-plugin/plugin.json`, la que sube cada
  versión) y toma las mejoras de `CHANGELOG.md`. Imprime `STATUS=up_to_date`
  o `STATUS=update_available` con las mejoras de cada versión nueva. No
  depende de que exista un release de GitHub y no instala nada.
- `SKILL.md` — la regla de lectura por partes, adaptada del skill de
  Claude Code. Incluye la tarea de **revisar actualizaciones una vez por
  semana**: si hay versión nueva, el agente le avisa al usuario con la
  versión y sus mejoras, y le pregunta si la instala. Nunca actualiza sin
  aprobación.

## Cómo se actualiza

Nada que hacer aquí: el instalador y el skill se jalan directo de GitHub por
URL cruda, y los binarios son los mismos del plugin de Claude Code
(`plugins/claude-code/native/`), que compila el workflow **Binarios** con
cada versión.

## Probado

2026-11-14 en la VM de Muse: `docpack 0.4.1` instalado, pack construido
de un .docx real (2 secciones, 8 tablas, 4 imágenes de página, estimado
1,499 tokens). Los packs quedan en `<carpeta-del-doc>/docpack/<archivo>/`.

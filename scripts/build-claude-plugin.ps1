# Prueba el motor, lo compila en release y deja el ejecutable dentro del plugin
# de Claude Code. Se detiene si la versión del motor y la del plugin no coinciden:
# el plugin fija su versión y los packs se reconstruyen cuando cambia, así que
# las dos tienen que ir juntas.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$engine = Join-Path $root 'engine'
$plugin = Join-Path $root 'plugins\claude-code'

$crate = (Select-String -Path (Join-Path $engine 'Cargo.toml') -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$manifest = (Get-Content -Raw (Join-Path $plugin '.claude-plugin\plugin.json') | ConvertFrom-Json).version
if ($crate -ne $manifest) { throw "Versiones distintas: engine/Cargo.toml $crate, plugin.json $manifest" }

cargo test --locked --manifest-path (Join-Path $engine 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Fallaron las pruebas del motor' }
cargo build --release --locked --manifest-path (Join-Path $engine 'Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Falló la compilación' }

$bin = Join-Path $plugin 'bin'
New-Item -ItemType Directory -Force $bin | Out-Null
Copy-Item (Join-Path $engine 'target\release\docpack.exe') (Join-Path $bin 'docpack.exe') -Force

$reported = (& (Join-Path $bin 'docpack.exe') --version).Trim()
if ($reported -ne "docpack $crate") { throw "El ejecutable dice '$reported', se esperaba 'docpack $crate'" }
$sha = (Get-FileHash (Join-Path $bin 'docpack.exe') -Algorithm SHA256).Hash.ToLower()
Write-Output "Plugin listo: $reported · SHA-256 $sha"

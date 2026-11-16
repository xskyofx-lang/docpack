# DocPack installer for Windows (PowerShell 5.1 or 7).
#   irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/xskyofx-lang/docpack/main/install/install.ps1))) -Agent codex
# Downloads docpack.exe into %LOCALAPPDATA%\Programs\DocPack and adds that
# folder to the user PATH. With -Agent (codex, kimi, antigravity, deepseek) it
# also installs that agent's skill and registers the `docpack mcp` server in
# its configuration (a copy of the old configuration is kept next to it).
# Safe to run again.
param(
    [ValidateSet('', 'codex', 'kimi', 'antigravity', 'deepseek')]
    [string]$Agent = '',
    # Leave the user PATH as it is (the agents get the full path anyway).
    [switch]$NoPath
)
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Raw = 'https://raw.githubusercontent.com/xskyofx-lang/docpack/main'
$BinDir = Join-Path $env:LOCALAPPDATA 'Programs\DocPack'
$Bin = Join-Path $BinDir 'docpack.exe'

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
Write-Host 'DocPack: descargando windows-x64 ...'
$download = "$Bin.download"
Invoke-WebRequest -UseBasicParsing -Uri "$Raw/plugins/claude-code/bin/docpack.exe" -OutFile $download
Move-Item -Force $download $Bin
& $Bin --version

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not $NoPath -and -not (($userPath -split ';') -contains $BinDir)) {
    $newPath = if ([string]::IsNullOrEmpty($userPath)) { $BinDir } else { "$userPath;$BinDir" }
    [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    Write-Host "Agregué $BinDir a tu PATH de usuario (abre una terminal nueva para usar 'docpack')."
}

if (-not $Agent) { Write-Host "DocPack instalado en $Bin"; return }

# Skill: one folder per skill with its SKILL.md, where each agent looks for it.
$skillDir = if ($Agent -eq 'antigravity') { Join-Path $HOME '.gemini\config\skills\docpack' } else { Join-Path $HOME '.agents\skills\docpack' }
New-Item -ItemType Directory -Force -Path $skillDir | Out-Null
Invoke-WebRequest -UseBasicParsing -Uri "$Raw/plugins/$Agent/SKILL.md" -OutFile (Join-Path $skillDir 'SKILL.md')
Write-Host "Skill instalada en $skillDir\SKILL.md"

# Adds docpack to the "mcpServers" of a JSON configuration, keeping the rest.
function Register-Json([string]$File) {
    New-Item -ItemType Directory -Force -Path (Split-Path $File) | Out-Null
    $data = [pscustomobject]@{}
    if ((Test-Path $File) -and (Get-Item $File).Length -gt 0) {
        Copy-Item -Force $File "$File.antes-de-docpack"
        $data = Get-Content -Raw -Encoding UTF8 $File | ConvertFrom-Json
    }
    if (-not $data.PSObject.Properties['mcpServers']) { $data | Add-Member -NotePropertyName mcpServers -NotePropertyValue ([pscustomobject]@{}) }
    $entry = [pscustomobject]@{ command = $Bin; args = @('mcp') }
    if ($data.mcpServers.PSObject.Properties['docpack']) { $data.mcpServers.docpack = $entry }
    else { $data.mcpServers | Add-Member -NotePropertyName docpack -NotePropertyValue $entry }
    # UTF-8 without BOM: the agents read it as JSON.
    [IO.File]::WriteAllText($File, ($data | ConvertTo-Json -Depth 32), (New-Object Text.UTF8Encoding $false))
    Write-Host "Servidor MCP docpack registrado en $File"
}

switch ($Agent) {
    'codex' {
        if (Get-Command codex -ErrorAction SilentlyContinue) {
            try { codex mcp remove docpack *> $null } catch { }
            codex mcp add docpack -- $Bin mcp
            Write-Host 'Servidor MCP docpack registrado en Codex (codex mcp list para verlo)'
        } else {
            Write-Host "No encontré el comando codex. Cuando lo tengas: codex mcp add docpack -- `"$Bin`" mcp"
        }
    }
    'kimi' {
        $kimiHome = if ($env:KIMI_CODE_HOME) { $env:KIMI_CODE_HOME } else { Join-Path $HOME '.kimi-code' }
        Register-Json (Join-Path $kimiHome 'mcp.json')
    }
    'antigravity' { Register-Json (Join-Path $HOME '.gemini\config\mcp_config.json') }
    'deepseek' {
        Write-Host 'DeepSeek no tiene un archivo de configuración único: cada cliente (DeepSeek-TUI, OpenCode, Cline...) tiene el suyo.'
        Write-Host 'Agrega este servidor en la configuración MCP de tu cliente:'
        Write-Host ('  "docpack": { "command": "' + ($Bin -replace '\\', '\\') + '", "args": ["mcp"] }')
    }
}
Write-Host "Listo. Reinicia $Agent para que cargue la skill y el servidor MCP."

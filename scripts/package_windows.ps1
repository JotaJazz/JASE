# Empaqueta el binario de Rust para Windows: deja dist\JASE, JASE-<version>-win64.zip y
# JASE-<version>-setup.exe en la raiz del repo, y escribe la version como ultima linea.
# Uso: ./scripts/package_windows.ps1 [-Iscc <ruta a ISCC.exe>]
# Solo ASCII: PowerShell 5 lee sin BOM como ANSI.
# PowerShell 7 (el de package.yml): el Compress-Archive de Windows PowerShell 5.1 escribe
# entradas con "\" en vez de "/".
#Requires -Version 7.0
param(
    [string]$Iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# -Iscc, relativo a donde se llama; el resto, a la raiz del repo.
$Iscc = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Iscc)
Set-Location (Join-Path $PSScriptRoot '..')
$root = (Get-Location).Path

# Desde rust\: asi aplica rust\.cargo\config.toml (CRT estatico).
Push-Location rust
try {
    $pkgid = cargo pkgid -p jase
    if ($LASTEXITCODE -ne 0) { throw "cargo pkgid fallo (codigo $LASTEXITCODE)" }
    $version = "$pkgid" -replace '^.*[#@]', ''
    if (-not $version) { throw "No se pudo leer la version de '$pkgid'" }

    # Out-Host: la unica salida del script es la version.
    cargo build --release --locked -p jase | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "cargo build fallo (codigo $LASTEXITCODE)" }
} finally {
    Pop-Location
}
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'rust\target' }
$bin = Join-Path $target 'release\jase.exe'

# dist\JASE sirve al zip y al instalador. _internal\README.txt (D3): el script portatil de
# 1.3.0 copia _internal primero y, si no esta en el zip, restaura la version vieja.
$dist = Join-Path $root 'dist\JASE'
if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
New-Item -ItemType Directory -Force (Join-Path $dist '_internal') | Out-Null
New-Item -ItemType Directory -Force (Join-Path $dist 'licenses') | Out-Null
# SelfUpdate::detect y jase.iss esperan JASE.exe.
Copy-Item $bin (Join-Path $dist 'JASE.exe')
Copy-Item packaging\windows\internal-README.txt (Join-Path $dist '_internal\README.txt')
foreach ($f in 'JASE-LICENSE.txt', 'DB-IP.txt', 'flag-icons-MIT.txt') {
    Copy-Item (Join-Path packaging\licenses $f) (Join-Path $dist 'licenses')
}
Push-Location rust
try {
    cargo about generate -c about.toml about.hbs -o (Join-Path $dist 'licenses\THIRD-PARTY.txt')
    if ($LASTEXITCODE -ne 0) { throw "cargo about fallo (codigo $LASTEXITCODE)" }
} finally {
    Pop-Location
}

# Exe de ventana: --smoke solo llega a stdout redirigido. tiny-skia: el runner no tiene GPU.
$out = Join-Path ([System.IO.Path]::GetTempPath()) "jase-smoke-$PID.txt"
$env:ICED_BACKEND = 'tiny-skia'
try {
    $p = Start-Process (Join-Path $dist 'JASE.exe') -ArgumentList '--smoke' `
        -RedirectStandardOutput $out -PassThru
    # 5 min, como el paso de humo de la 1.3.0: un cuelgue no gasta el plazo del job.
    if (-not $p.WaitForExit(300000)) {
        $p.Kill($true)
        throw 'smoke timeout'
    }
    $lines = @(Get-Content $out)
} finally {
    Remove-Item Env:ICED_BACKEND
    Remove-Item $out -ErrorAction SilentlyContinue
}
Write-Host "smoke exit=$($p.ExitCode) result=$($lines -join ' | ')"
if ($p.ExitCode -ne 0 -or $lines -notcontains "JASE smoke OK $version") {
    throw "El exe empaquetado fallo el smoke test"
}

$zip = "JASE-$version-win64.zip"
Compress-Archive -Path dist\JASE -DestinationPath $zip -Force

if (-not (Test-Path $Iscc)) { throw "No se encontro ISCC en $Iscc" }
& $Iscc "/DAppVersion=$version" "/DSourceDir=$dist" "/DOutputDir=$root" packaging\jase.iss |
    Out-Host
if ($LASTEXITCODE -ne 0) { throw "ISCC fallo (codigo $LASTEXITCODE)" }
if (-not (Test-Path "JASE-$version-setup.exe")) { throw "No se genero JASE-$version-setup.exe" }

$version

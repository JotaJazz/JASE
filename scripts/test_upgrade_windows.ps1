# Prueba la actualizacion desde la 1.3.0 publicada (D9): instalacion limpia, instalador
# nuevo sobre una 1.3.0 instalada y zip portatil sobre portatil con el portable_update.py
# de la tag v1.3.0. Necesita los paquetes de package_windows.ps1, git con la tag y python.
# Uso: ./scripts/test_upgrade_windows.ps1 -Version <version> [-Ci]
# Sin -Ci, las pruebas del instalador se saltan si ya hay una JASE instalada para el
# usuario: instalan y desinstalan con el mismo AppId y pisarian la instalacion real.
# Solo ASCII: PowerShell 5 lee sin BOM como ANSI.
#Requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$Version,
    [switch]$Ci
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Set-Location (Join-Path $PSScriptRoot '..')
$root = (Get-Location).Path

$OldSetupUrl = 'https://github.com/JotaJazz/JASE/releases/download/v1.3.0/JASE-1.3.0-setup.exe'
$OldZipUrl = 'https://github.com/JotaJazz/JASE/releases/download/v1.3.0/JASE-1.3.0-win64.zip'
# Los digest de la release v1.3.0.
$OldSetupSha = '48029af828b5f60b6fc1bc8d8f59917808bc43e133d6a832336a4bedf50f61ec'
$OldZipSha = '5314b3615f27d307e7c6794ad9b4c32c92f3d5d6f9b143b1ba30d4baca57db80'
$UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{5A639811-DD43-4C26-AF69-0EFDE699CB93}_is1'
$ProtocolKey = 'HKCU:\Software\Classes\jase'
# Una JASE ya instalada, para el usuario o para todos: sin -Ci, no se toca.
$InstalledKeys = @(
    $UninstallKey,
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{5A639811-DD43-4C26-AF69-0EFDE699CB93}_is1',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{5A639811-DD43-4C26-AF69-0EFDE699CB93}_is1'
)
# Lo instalo esta corrida y aun no lo desinstalo: el finally lo quita si algo falla.
$script:Installed = $false

$NewSetup = Join-Path $root "JASE-$Version-setup.exe"
$NewZip = Join-Path $root "JASE-$Version-win64.zip"
$DistExe = Join-Path $root 'dist\JASE\JASE.exe'
foreach ($f in $NewSetup, $NewZip, $DistExe) {
    if (-not (Test-Path -LiteralPath $f)) { throw "Falta $f (correr antes package_windows.ps1)" }
}

# Corre $File y espera hasta $Seconds; si vence, mata el arbol y falla (un cuelgue no gasta
# el plazo del job). Sin -Wait: Start-Process -Wait espera tambien a los hijos (JASE).
function Invoke-Waited([string]$File, [string]$Arguments, [int]$Seconds, [string]$Out) {
    $extra = @{}
    if ($Out) { $extra.RedirectStandardOutput = $Out }
    $p = Start-Process $File -ArgumentList $Arguments -PassThru @extra
    $null = $p.Handle  # sin el handle, ExitCode puede quedar vacio tras WaitForExit
    if (-not $p.WaitForExit($Seconds * 1000)) {
        $p.Kill($true)
        throw "$File $Arguments no termino en $Seconds s"
    }
    $p.ExitCode
}

# Exe de ventana: --smoke solo llega a stdout redirigido.
function Invoke-Smoke([string]$Exe, [string]$Want) {
    $out = Join-Path $env:TEMP "smoke-$([guid]::NewGuid()).txt"
    $code = Invoke-Waited $Exe '--smoke' 300 $out
    $lines = @(Get-Content -LiteralPath $out)
    Write-Host "smoke $Exe exit=$code result=$($lines -join ' | ')"
    if ($code -ne 0 -or $lines -notcontains "JASE smoke OK $Want") {
        throw "$Exe no paso el smoke de la version $Want"
    }
}

function Install-Setup([string]$Setup, [string]$Dir, [string]$Log) {
    $a = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER'
    if ($Dir) { $a += " `"/DIR=$Dir`"" }
    $a += " `"/LOG=$Log`""
    $code = Invoke-Waited $Setup $a 300
    if ($code -ne 0) {
        if (Test-Path -LiteralPath $Log) { Get-Content -LiteralPath $Log | Out-Host }
        throw "$Setup fallo (codigo $code)"
    }
    $script:Installed = $true
}

# Espera hasta 60 s a que no quede ninguna de $Paths; devuelve las que quedan.
function Wait-Gone([string[]]$Paths) {
    $deadline = (Get-Date).AddSeconds(60)
    while (@($Paths | Where-Object { Test-Path -LiteralPath $_ }).Count -and (Get-Date) -lt $deadline) {
        Start-Sleep -Seconds 1
    }
    @($Paths | Where-Object { Test-Path -LiteralPath $_ })
}

function Uninstall-Jase([string]$Dir) {
    $code = Invoke-Waited (Join-Path $Dir 'unins000.exe') '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' 300
    if ($code -ne 0) { throw "unins000.exe fallo (codigo $code)" }
    # El desinstalador corre desde una copia temporal: esperar a que termine de borrar.
    $rest = @(Wait-Gone @((Join-Path $Dir 'JASE.exe'), (Join-Path $Dir '_internal'), $UninstallKey, $ProtocolKey))
    if ($rest.Count) { throw "La desinstalacion dejo: $($rest -join ', ')" }
    $script:Installed = $false
}

function Assert-Protocol([string]$Dir) {
    $cmd = (Get-ItemProperty -LiteralPath "$ProtocolKey\shell\open\command").'(default)'
    $want = "`"$Dir\JASE.exe`" `"%1`""
    if ($cmd -ne $want) { throw "jase:// abre '$cmd', se esperaba '$want'" }
}

# _internal solo con README.txt: los archivos de PyInstaller ya no estan.
function Assert-OnlyReadme([string]$Internal) {
    $names = @(Get-ChildItem -LiteralPath $Internal -Force -Recurse |
            ForEach-Object { $_.FullName.Substring($Internal.Length + 1) })
    if ($names.Count -ne 1 -or $names[0] -ne 'README.txt') {
        throw "$Internal trae: $($names -join ', ')"
    }
}

function Get-Verified([string]$Url, [string]$Sha, [string]$Dir) {
    $file = Join-Path $Dir ($Url -replace '^.*/', '')
    Invoke-WebRequest $Url -OutFile $file
    $got = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash
    if ($got -ne $Sha) { throw "SHA-256 de $Url es $got, se esperaba $Sha" }
    $file
}

function Test-FreshInstall {
    $d = Join-Path $work 'fresh'
    Install-Setup $NewSetup $d (Join-Path $work 'fresh-setup.log')
    Invoke-Smoke (Join-Path $d 'JASE.exe') $Version
    Assert-Protocol $d
    $cfg = Join-Path $env:APPDATA 'JimmysASE\config.json'
    New-Item -ItemType Directory -Force (Split-Path $cfg) | Out-Null
    Set-Content -LiteralPath $cfg -Value '{}'
    Uninstall-Jase $d
    if (-not (Test-Path -LiteralPath $cfg)) { throw "La desinstalacion borro $cfg" }
    Write-Host 'OK: instalacion limpia'
}

function Test-InstallerOver130 {
    $d = Join-Path $work 'installed'
    Install-Setup $oldSetup $d (Join-Path $work 'old-setup.log')
    if (-not (Test-Path -LiteralPath (Join-Path $d '_internal\PySide6'))) {
        throw "La 1.3.0 no dejo $d\_internal\PySide6"
    }
    $cfg = Join-Path $env:APPDATA 'JimmysASE\config.json'
    New-Item -ItemType Directory -Force (Split-Path $cfg) | Out-Null
    Set-Content -LiteralPath $cfg -Value '{"upgrade": 130}'
    # Sin /DIR, como el actualizador de 1.3.0: el mismo AppId reusa la carpeta anterior.
    Install-Setup $NewSetup '' (Join-Path $work 'new-setup.log')
    Invoke-Smoke (Join-Path $d 'JASE.exe') $Version
    Assert-OnlyReadme (Join-Path $d '_internal')
    $keys = @(Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' |
            Where-Object { $_.PSChildName -like '*5A639811-DD43-4C26-AF69-0EFDE699CB93*' })
    if ($keys.Count -ne 1) { throw "Hay $($keys.Count) claves de desinstalacion de JASE" }
    $u = Get-ItemProperty -LiteralPath $UninstallKey
    if ($u.DisplayVersion -ne $Version) { throw "DisplayVersion=$($u.DisplayVersion), se esperaba $Version" }
    if ($u.InstallLocation -ne "$d\") { throw "InstallLocation=$($u.InstallLocation), se esperaba $d\" }
    Assert-Protocol $d
    if ((Get-Content -LiteralPath $cfg -Raw).Trim() -ne '{"upgrade": 130}') { throw "La actualizacion cambio $cfg" }
    # D2: quedan los avisos viejos de licenses\ (CPython, PySide6); se anota, no falla.
    Write-Host "licenses tras actualizar: $((Get-ChildItem -LiteralPath (Join-Path $d 'licenses')).Name -join ', ')"
    Uninstall-Jase $d
    if (-not (Test-Path -LiteralPath $cfg)) { throw "La desinstalacion borro $cfg" }
    Write-Host 'OK: instalador sobre 1.3.0'
}

function Test-PortableOver130 {
    $old = Join-Path $work 'portable'
    Expand-Archive -LiteralPath $oldZip -DestinationPath $old
    $dst = Join-Path $old 'JASE'
    if (-not (Test-Path -LiteralPath (Join-Path $dst '_internal\PySide6'))) {
        throw "El zip de 1.3.0 no dejo $dst\_internal\PySide6"
    }
    # Un PID ya terminado: el script no espera a ninguna JASE.
    $deadPid = (Start-Process cmd.exe '/c exit' -Wait -PassThru).Id

    # El codigo de 1.3.0 tal cual, como lo llama window_background.py.
    $py = Join-Path $work 'py'
    New-Item -ItemType Directory -Force $py | Out-Null
    $src = git show v1.3.0:src/jase/portable_update.py
    if ($LASTEXITCODE -ne 0) { throw "git show fallo (codigo $LASTEXITCODE): falta la tag v1.3.0?" }
    Set-Content -LiteralPath (Join-Path $py 'portable_update_130.py') -Value $src -Encoding utf8NoBOM
    $cache = Join-Path $work 'cache'
    $pyCode = 'import sys; from pathlib import Path; sys.path.insert(0, sys.argv[1]); ' +
    'import portable_update_130 as p; new = p.extract_release(Path(sys.argv[2]), Path(sys.argv[3])); ' +
    'print(p.write_updater(Path(sys.argv[4]), int(sys.argv[5]), new, Path(sys.argv[6])))'
    $script = python -I -c $pyCode $py $NewZip (Join-Path $cache "portable-JASE-$Version-win64.zip") $cache $deadPid $dst
    if ($LASTEXITCODE -ne 0) { throw "python fallo (codigo $LASTEXITCODE)" }
    $script = "$script".Trim()
    Write-Host "script: $script"

    # cmd.exe /c, como QProcess.startDetached; si falla la copia, el script hace pause.
    $code = Invoke-Waited cmd.exe "/c `"$script`"" 300
    Write-Host "script exit=$code"
    $exe = Join-Path $dst 'JASE.exe'
    if ((Get-FileHash -LiteralPath $exe).Hash -ne (Get-FileHash -LiteralPath $DistExe).Hash) {
        throw "$exe no es el exe nuevo"
    }
    Assert-OnlyReadme (Join-Path $dst '_internal')
    if (Test-Path -LiteralPath (Join-Path $dst '_internal.old')) { throw "Quedo $dst\_internal.old" }
    if (Test-Path -LiteralPath $script) { throw "El script $script no se borro" }
    # El start "" del script relanza JASE: debe seguir viva.
    Start-Sleep -Seconds 10
    $alive = @(Get-Process JASE -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe })
    if (-not $alive.Count) { throw "JASE no quedo abierta tras actualizar el portatil" }
    $alive | Stop-Process -Force
    Write-Host 'OK: portatil sobre 1.3.0'
}

# LOCALAPPDATA y no GetTempPath(): TEMP puede venir en forma 8.3 (JOTA~1) y las rutas de
# los procesos y del registro se comparan con la forma larga.
$base = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { Join-Path $env:LOCALAPPDATA 'Temp' }
$work = Join-Path $base "jase-upgrade-$PID"
$saved = @{}
foreach ($n in 'APPDATA', 'LOCALAPPDATA', 'TEMP', 'TMP', 'ICED_BACKEND') {
    $saved[$n] = [Environment]::GetEnvironmentVariable($n)
}
$savedEncoding = [Console]::OutputEncoding
try {
    # Carpetas del usuario de prueba: la config real del que corre el script no se toca.
    foreach ($n in 'APPDATA', 'LOCALAPPDATA', 'TEMP') {
        $dir = Join-Path $work "env\$n"
        New-Item -ItemType Directory -Force $dir | Out-Null
        [Environment]::SetEnvironmentVariable($n, $dir)
    }
    $env:TMP = $env:TEMP
    # tiny-skia: el runner no tiene GPU.
    $env:ICED_BACKEND = 'tiny-skia'
    # git show en UTF-8: el .py de 1.3.0 tiene tildes.
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)

    $dl = Join-Path $work 'download'
    New-Item -ItemType Directory -Force $dl | Out-Null
    $oldSetup = Get-Verified $OldSetupUrl $OldSetupSha $dl
    $oldZip = Get-Verified $OldZipUrl $OldZipSha $dl

    if ($Ci -or -not @($InstalledKeys | Where-Object { Test-Path -LiteralPath $_ }).Count) {
        Test-FreshInstall
        Test-InstallerOver130
    } else {
        Write-Warning 'Ya hay una JASE instalada para este usuario: se saltan las pruebas del instalador (usar -Ci para forzarlas).'
    }
    Test-PortableOver130
} finally {
    # Si algo fallo a medias: la JASE relanzada o un smoke colgado tienen archivos de $work.
    Get-Process JASE -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$work\*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    # Solo lo que instalo esta corrida: una clave que ya estaba nunca se toca.
    if ($script:Installed -and (Test-Path -LiteralPath $UninstallKey)) {
        try {
            $loc = (Get-ItemProperty -LiteralPath $UninstallKey).InstallLocation
            $null = Invoke-Waited (Join-Path $loc 'unins000.exe') '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' 300
            $null = Wait-Gone @($UninstallKey)
        } catch {
            Write-Warning "No se pudo desinstalar la JASE de prueba: $_"
        }
        foreach ($k in $UninstallKey, $ProtocolKey) {
            Remove-Item -LiteralPath $k -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    [Console]::OutputEncoding = $savedEncoding
    foreach ($n in $saved.Keys) { [Environment]::SetEnvironmentVariable($n, $saved[$n]) }
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
Write-Host 'Actualizacion desde 1.3.0: OK'

; Instalador de JASE (Inno Setup 6).
; Este archivo es solo ASCII a proposito: ISCC lo lee sin depender de la codificacion.
;
; Compilar (CI, ver .github/workflows/release.yml):
;   ISCC.exe /DAppVersion=0.9.0 /DSourceDir=C:\ruta\dist\JASE /DOutputDir=C:\ruta\salida packaging\jase.iss
; Las rutas relativas se resuelven desde la carpeta de este archivo (packaging\).

#ifndef AppVersion
  #error Falta la version: compilar con /DAppVersion=<version>
#endif
#ifndef SourceDir
  #define SourceDir "..\dist\JASE"
#endif
#ifndef OutputDir
  #define OutputDir "..\dist\installer"
#endif

#define AppName "Jimmy's All Seeing Eye"
#define AppUrl "https://github.com/JotaJazz/JASE"

[Setup]
; AppId FIJO: identifica la app en Windows (Agregar o quitar programas) y permite que
; una version nueva actualice a la anterior en vez de instalarse al lado. Generado una
; sola vez; NUNCA cambiarlo. La llave doble escapa la llave inicial.
AppId={{5A639811-DD43-4C26-AF69-0EFDE699CB93}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
VersionInfoVersion={#AppVersion}
AppPublisher=JotaJazz
AppPublisherURL={#AppUrl}
AppSupportURL={#AppUrl}
AppUpdatesURL={#AppUrl}
DefaultDirName={autopf}\JASE
DisableProgramGroupPage=yes
; Sin admin por defecto (instala para el usuario actual); el dialogo permite elegir
; instalar para todos los usuarios. /ALLUSERS y /CURRENTUSER tambien sirven en modo silencioso.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\src\jase\assets\app_icon.ico
UninstallDisplayIcon={app}\JASE.exe
LicenseFile=..\LICENSE
WizardStyle=modern
OutputDir={#OutputDir}
OutputBaseFilename=JASE-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes

; Datos del usuario: la configuracion, la cache y los masters propios viven en
; %APPDATA%\JimmysASE y la desinstalacion NO los borra (Inno solo quita lo que instalo).
; Es intencional: no agregar una seccion de borrado al desinstalar para esa carpeta.

[InstallDelete]
; Al actualizar, los restos de PyInstaller de la version anterior (DLL o modulos que ya
; no se usan) pueden romper el arranque: se borra solo {app}\_internal antes de copiar.
Type: filesandordirs; Name: "{app}\_internal"

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; Toda la carpeta de PyInstaller (onedir), incluida licenses\.
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\JASE"; Filename: "{app}\JASE.exe"
Name: "{autodesktop}\JASE"; Filename: "{app}\JASE.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\JASE.exe"; Description: "{cm:LaunchProgram,JASE}"; Flags: nowait postinstall skipifsilent

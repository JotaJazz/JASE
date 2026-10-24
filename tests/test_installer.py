import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ISS = ROOT / "packaging" / "jase.iss"
APP_ID = "5A639811-DD43-4C26-AF69-0EFDE699CB93"


def _iss() -> str:
    return ISS.read_text(encoding="utf-8")


def _directive(text: str, name: str) -> str:
    match = re.search(rf"^{name}=(.*)$", text, re.M)
    assert match, f"falta {name}"
    return match.group(1).strip()


def test_iss_fixed_app_id():
    text = _iss()
    # Las llaves dobles escapan la llave inicial en Inno: AppId={{GUID}.
    assert _directive(text, "AppId") == "{{" + APP_ID + "}"


def test_iss_identity_and_urls():
    text = _iss()
    assert '#define AppName "Jimmy\'s All Seeing Eye"' in text
    assert _directive(text, "AppName") == "{#AppName}"
    assert "{#AppVersion}" in _directive(text, "AppVerName")
    assert _directive(text, "AppVersion") == "{#AppVersion}"
    assert _directive(text, "AppPublisher") == "JotaJazz"
    assert '#define AppUrl "https://github.com/JotaJazz/JASE"' in text
    for key in ("AppPublisherURL", "AppSupportURL", "AppUpdatesURL"):
        assert _directive(text, key) == "{#AppUrl}"


def test_iss_command_line_defines():
    text = _iss()
    assert re.search(r"^#ifndef AppVersion\s*\n\s*#error", text, re.M)
    assert re.search(r"^#ifndef SourceDir$", text, re.M)
    assert re.search(r"^#ifndef OutputDir$", text, re.M)
    assert _directive(text, "OutputDir") == "{#OutputDir}"
    assert _directive(text, "OutputBaseFilename") == "JASE-{#AppVersion}-setup"


def test_iss_install_mode_and_arch():
    text = _iss()
    assert _directive(text, "DefaultDirName") == r"{autopf}\JASE"
    assert _directive(text, "PrivilegesRequired") == "lowest"
    assert _directive(text, "PrivilegesRequiredOverridesAllowed") == "dialog"
    assert _directive(text, "ArchitecturesAllowed") == "x64compatible"
    assert _directive(text, "ArchitecturesInstallIn64BitMode") == "x64compatible"
    assert _directive(text, "WizardStyle") == "modern"


def test_iss_icon_and_license_exist():
    text = _iss()
    icon = _directive(text, "SetupIconFile")
    assert icon == r"..\src\jase\assets\app_icon.ico"
    assert (ISS.parent / icon.replace("\\", "/")).is_file()
    lic = _directive(text, "LicenseFile")
    assert lic == r"..\LICENSE"
    assert (ISS.parent / lic.replace("\\", "/")).is_file()
    assert _directive(text, "UninstallDisplayIcon") == r"{app}\JASE.exe"


def test_iss_installs_whole_folder():
    text = _iss()
    assert re.search(
        r'^Source: "\{#SourceDir\}\\\*"; DestDir: "\{app\}"; Flags: .*recursesubdirs.*createallsubdirs',
        text,
        re.M,
    )


def test_iss_languages():
    text = _iss()
    assert re.search(r'^Name: "english"; MessagesFile: "compiler:Default\.isl"$', text, re.M)
    assert re.search(r'^Name: "spanish"; MessagesFile: "compiler:Languages\\Spanish\.isl"$', text, re.M)


def test_iss_shortcuts_and_launch():
    text = _iss()
    tasks = re.search(r"^\[Tasks\]\n(.*?)(?=^\[)", text, re.M | re.S).group(1)
    desktop = [line for line in tasks.splitlines() if line.startswith('Name: "desktopicon"')]
    assert len(desktop) == 1 and "Flags: unchecked" in desktop[0]
    icons = re.search(r"^\[Icons\]\n(.*?)(?=^\[)", text, re.M | re.S).group(1)
    assert re.search(r'^Name: "\{autoprograms\}\\JASE"; Filename: "\{app\}\\JASE\.exe"$', icons, re.M)
    assert re.search(r'^Name: "\{autodesktop\}\\JASE"; .*Tasks: desktopicon$', icons, re.M)
    run = re.search(r"^\[Run\]\n(.*)", text, re.M | re.S).group(1)
    assert r'Filename: "{app}\JASE.exe"' in run
    assert "postinstall" in run and "skipifsilent" in run and "nowait" in run


def _section(text: str, name: str) -> list[str]:
    match = re.search(rf"^\[{name}\]\n(.*?)(?=^\[|\Z)", text, re.M | re.S)
    if not match:
        return []
    return [line for line in match.group(1).splitlines() if line.strip() and not line.lstrip().startswith(";")]


def test_iss_never_deletes_user_data():
    text = _iss()
    for name in ("InstallDelete", "UninstallDelete"):
        for line in _section(text, name):
            assert "JimmysASE" not in line and "{userappdata}" not in line and "{appdata}" not in line
    code = [line for line in text.splitlines() if not line.lstrip().startswith(";")]
    assert not any("JimmysASE" in line for line in code)
    # El motivo queda documentado en un comentario.
    assert any("JimmysASE" in line for line in text.splitlines() if line.lstrip().startswith(";"))


def test_iss_install_delete_only_stale_internal():
    # Restos de PyInstaller de una versión anterior rompen la actualización: se borra _internal.
    entries = _section(_iss(), "InstallDelete")
    assert entries == ['Type: filesandordirs; Name: "{app}\\_internal"']
    assert _section(_iss(), "UninstallDelete") == []


def test_iss_version_info_and_no_group():
    text = _iss()
    assert _directive(text, "VersionInfoVersion") == "{#AppVersion}"
    assert not re.search(r"^DefaultGroupName=", text, re.M)


def test_iss_is_ascii():
    # ISCC lee los .iss sin BOM como ANSI: solo ASCII evita depender de la codificación.
    assert ISS.read_bytes().isascii()


def test_iss_registers_jase_protocol():
    lines = _section(_iss(), "Registry")
    assert lines and all(line.startswith("Root: HKA;") for line in lines)
    joined = "\n".join(lines)
    assert 'Subkey: "Software\\Classes\\jase"; ValueType: string; ValueName: ""; ValueData: "URL:JASE"' in joined
    assert 'ValueName: "URL Protocol"; ValueData: ""' in joined
    assert 'ValueData: "{app}\\JASE.exe,0"' in joined
    assert 'ValueData: """{app}\\JASE.exe"" ""%1"""' in joined
    # uninsdeletekey solo en la clave raiz jase (borra las subclaves).
    flagged = [line for line in lines if "uninsdeletekey" in line]
    assert len(flagged) == 1 and 'Subkey: "Software\\Classes\\jase";' in flagged[0]

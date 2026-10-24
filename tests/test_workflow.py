import re
from pathlib import Path

from jase.games import GAMES

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
NOTES = ROOT / "packaging" / "release-notes.md"


def _wf() -> str:
    return WORKFLOW.read_text(encoding="utf-8")


def test_workflow_triggers():
    text = _wf()
    assert re.search(r"^on:\s*\n\s+push:\s*\n\s+tags:\s*\n\s+- ['\"]?v\*['\"]?\s*$", text, re.M)
    assert re.search(r"^\s+workflow_dispatch:", text, re.M)
    assert re.search(r"^permissions:\s*\n\s+contents: read\s*$", text, re.M)
    assert text.count("contents: write") == 1
    assert "windows-latest" in text


def test_workflow_runs_tests_and_smoke():
    text = _wf()
    assert "pytest -q -W error" in text
    assert "QT_QPA_PLATFORM: offscreen" in text
    assert "pyinstaller jase.spec" in text
    assert "'--smoke'" in text and "SMOKE OK" in text
    assert "packaging/licenses" in text and r"dist\JASE\licenses" in text
    assert "Compress-Archive" in text and "JASE-${{ steps.ver.outputs.version }}-win64.zip" in text
    assert "pip install -e" not in text  # metadata real: instalación no editable
    # Orden: pruebas, build, smoke, licencias, zip.
    order = ["pytest -q -W error", "pyinstaller jase.spec", "--smoke", "packaging/licenses", "Compress-Archive"]
    pos = [text.index(s) for s in order]
    assert pos == sorted(pos)


def test_workflow_release_is_draft_prerelease():
    text = _wf()
    assert re.search(r"^\s+draft: true\s*$", text, re.M)
    assert re.search(r"^\s+prerelease: true\s*$", text, re.M)
    assert "body_path: packaging/release-notes.md" in text
    assert "softprops/action-gh-release@" in text
    assert "startsWith(github.ref, 'refs/tags/v')" in text
    assert "upload-artifact@" in text


def test_workflow_pins_versions():
    text = _wf()
    assert 'python-version: "3.13"' in text
    assert "PySide6==6.12.0" in text
    assert re.search(r"\.\[build,dev\]", text)
    pyproject = (ROOT / "pyproject.toml").read_text(encoding="utf-8")
    assert re.search(r"pyinstaller==\d+\.\d+\.\d+", pyproject)
    uses = re.findall(r"^\s+(?:-\s+)?uses:\s*(\S+)", text, re.M)
    assert uses
    for ref in uses:
        assert re.fullmatch(r"[\w./-]+@[0-9a-f]{40}", ref), f"acción sin SHA fijo: {ref}"


def test_release_notes_bilingual_and_lists_games():
    text = NOTES.read_text(encoding="utf-8")
    assert text.count("<!-- ES -->") == 1
    en, es = text.split("<!-- ES -->")
    for half in (en, es):
        for game in GAMES.values():
            assert game.name in half, game.name
        assert "SmartScreen" in half
        assert "%APPDATA%\\JimmysASE" in half
    assert "More info" in en and "Run anyway" in en
    assert "Más información" in es and "Ejecutar de todas formas" in es
    for needle in ("BF1942", "CS 1.6", "SoF2", "JKA"):
        assert needle in en


def test_workflow_has_timeouts():
    text = _wf()
    assert re.search(r"^    runs-on: windows-latest\n    timeout-minutes: 45$", text, re.M)
    assert re.search(r"^    runs-on: ubuntu-latest\n    timeout-minutes: 10$", text, re.M)
    assert re.search(r"name: Smoke test\n        timeout-minutes: 5$", text, re.M)


def test_release_notes_link_issues():
    en, es = NOTES.read_text(encoding="utf-8").split("<!-- ES -->")
    for half in (en, es):
        assert "https://github.com/JotaJazz/JASE/issues" in half


def test_workflow_smoke_uses_result_file():
    text = _wf()
    assert "JASE_SMOKE_OUT" in text
    assert "Start-Process dist\\JASE\\JASE.exe" in text and "-PassThru" in text and "-Wait" in text
    assert "ExitCode" in text
    assert "SMOKE OK ${{ steps.ver.outputs.version }}" in text


def test_workflow_checkouts_do_not_persist_credentials():
    text = _wf()
    assert text.count("actions/checkout@") == 2
    assert text.count("persist-credentials: false") == 2


def test_workflow_collects_third_party_licenses():
    text = _wf()
    assert r"python scripts/collect_licenses.py dist\JASE\licenses" in text
    assert text.index("collect_licenses.py") < text.index("Compress-Archive")


def test_workflow_pyinstaller_comment_matches_pin():
    pin = re.search(r"pyinstaller==(\d+\.\d+\.\d+)", (ROOT / "pyproject.toml").read_text()).group(1)
    assert f"pyinstaller=={pin}" in _wf()


INNO_VERSION = "6.7.1"


def _step(text: str, name: str) -> str:
    match = re.search(rf"^      - name: {re.escape(name)}\n(.*?)(?=^      - |^  \w|\Z)", text, re.M | re.S)
    assert match, f"falta el paso {name}"
    return match.group(1)


def test_workflow_installs_pinned_inno_setup():
    text = _wf()
    step = _step(text, "Install Inno Setup")
    assert f"choco install innosetup --version {INNO_VERSION} " in step
    assert "-y" in step and "--no-progress" in step
    assert r'"${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"' in step
    assert "Test-Path" in step
    assert re.search(r"timeout-minutes: \d+", step)
    # ISCC.exe no tiene recurso de versión: la versión se comprueba con choco.
    assert "ProductVersion" not in step
    assert "choco list innosetup --exact --limit-output" in step
    assert f"if ($v -ne '{INNO_VERSION}') {{ throw" in step and "'$v'" in step
    assert step.index("Test-Path") < step.index("choco list")
    # Códigos mejorados de choco: 0 = instalado, 2 = ya estaba instalado.
    assert "$LASTEXITCODE -notin 0, 2" in step


def test_workflow_compiles_installer():
    text = _wf()
    step = _step(text, "Build installer")
    assert '"/DAppVersion=${{ steps.ver.outputs.version }}"' in step
    assert "/DSourceDir=" in step and "/DOutputDir=" in step
    assert r"packaging\jase.iss" in step
    assert "$LASTEXITCODE" in step
    assert "JASE-${{ steps.ver.outputs.version }}-setup.exe" in step
    assert re.search(r"timeout-minutes: \d+", step)
    # Después del zip y antes de subir el artefacto.
    assert text.index("Compress-Archive") < text.index("name: Install Inno Setup")
    assert text.index("name: Installer smoke test") < text.index("upload-artifact@")


def test_workflow_installer_smoke_test():
    text = _wf()
    step = _step(text, "Installer smoke test")
    assert re.search(r"timeout-minutes: \d+", step)
    for flag in ("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/DIR="):
        assert flag in step
    assert step.count("-Wait -PassThru") >= 3
    assert "JASE_SMOKE_OUT" in step and "'--smoke'" in step
    assert "SMOKE OK ${{ steps.ver.outputs.version }}" in step
    assert "unins000.exe" in step
    assert step.count("ExitCode -ne 0") >= 3
    # Instala, prueba y desinstala, en ese orden.
    assert step.index("/CURRENTUSER") < step.index("'--smoke'") < step.index("unins000.exe")


def test_workflow_ships_zip_and_installer():
    text = _wf()
    zip_name = "JASE-${{ steps.ver.outputs.version }}-win64.zip"
    setup_name = "JASE-${{ steps.ver.outputs.version }}-setup.exe"
    upload = re.search(r"upload-artifact@.*?(?=^  \w)", text, re.M | re.S).group(0)
    assert zip_name in upload and setup_name in upload
    files = re.search(r"^\s+files: \|\n((?:\s{12}\S.*\n)+)", text, re.M)
    assert files, "files: debe ser multilínea"
    lines = [line.strip() for line in files.group(1).splitlines()]
    assert lines == [
        "JASE-${{ needs.build.outputs.version }}-win64.zip",
        "JASE-${{ needs.build.outputs.version }}-setup.exe",
    ]
    assert re.search(r"^\s+fail_on_unmatched_files: true\s*$", text, re.M)


def test_release_notes_mention_both_downloads():
    en, es = NOTES.read_text(encoding="utf-8").split("<!-- ES -->")
    for half in (en, es):
        assert "JASE-0.12.0-setup.exe" in half
        assert "JASE-0.12.0-win64.zip" in half
    assert "recommended" in en and "Start menu" in en and "uninstall" in en.lower()
    assert "recomendado" in es and "menú Inicio" in es and "desinstala" in es.lower()
    assert "no hay instalador" not in es and "there is no installer" not in en


def test_release_notes_version_matches_pyproject():
    import tomllib

    version = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))["project"]["version"]
    text = NOTES.read_text(encoding="utf-8")
    assert f"JASE-{version}-setup.exe" in text and f"JASE-{version}-win64.zip" in text


def test_release_notes_wording():
    en, es = NOTES.read_text(encoding="utf-8").split("<!-- ES -->")
    assert "if you choose so" not in en
    assert "an uninstaller in *Settings → Apps*" in en
    assert "un desinstalador en *Configuración → Aplicaciones*" in es


CI = ROOT / ".github" / "workflows" / "ci.yml"


def test_ci_triggers_and_jobs():
    text = CI.read_text(encoding="utf-8")
    assert re.search(r"^on:\s*\n\s+push:", text, re.M)
    assert re.search(r"^\s+pull_request:", text, re.M)
    assert re.search(r"^\s+branches:\s*\n\s+- ['\"]\*\*['\"]", text, re.M)
    assert "tags-ignore" not in text
    assert "persist-credentials: false" in text and "timeout-minutes: 30" in text
    assert re.search(r"^permissions:\s*\n\s+contents: read\s*$", text, re.M)
    assert "ubuntu-latest" in text and "windows-latest" in text
    assert "ruff check src tests scripts" in text
    assert "pytest -q -W error" in text and "QT_QPA_PLATFORM: offscreen" in text
    assert "PySide6==6.12.0" in text


def test_ci_pins_actions():
    uses = re.findall(r"^\s+(?:-\s+)?uses:\s*(\S+)", CI.read_text(encoding="utf-8"), re.M)
    assert uses
    for ref in uses:
        assert re.fullmatch(r"[\w./-]+@[0-9a-f]{40}", ref), ref


def test_ci_runs_mypy_on_linux_only():
    text = CI.read_text(encoding="utf-8")
    assert re.search(r"- name: Mypy\s*\n\s+if: runner\.os == 'Linux'\s*\n\s+run: mypy\s*$", text, re.M)


def test_dependabot_updates_actions_weekly():
    text = (ROOT / ".github" / "dependabot.yml").read_text(encoding="utf-8")
    assert re.search(r"^version: 2\s*$", text, re.M)
    assert text.count("package-ecosystem:") == 1
    assert re.search(r"package-ecosystem: ['\"]github-actions['\"]", text)
    assert re.search(r"directory: ['\"]/['\"]", text)
    assert re.search(r"interval: ['\"]weekly['\"]", text)

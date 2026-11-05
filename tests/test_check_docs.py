"""Verificador de la documentación: enlaces, rutas, datos personales y memoria."""
import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("check_docs", ROOT / "scripts" / "check_docs.py")
check_docs = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(check_docs)


def repo(tmp_path, files):
    for name, text in files.items():
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return tmp_path


def problems(tmp_path, files):
    return check_docs.check(repo(tmp_path, files))


def test_real_repository_is_clean():
    assert check_docs.check(ROOT) == []


def test_broken_relative_link(tmp_path):
    found = problems(tmp_path, {"docs/README.md": "[x](no-existe.md) [y](https://e.org) [z](#a)"})
    assert len(found) == 1 and "no-existe.md" in found[0]


def test_link_with_anchor_to_existing_file_is_fine(tmp_path):
    assert problems(tmp_path, {"docs/a.md": "x", "docs/README.md": "[a](a.md#seccion)"}) == []


def test_backtick_repo_path_must_exist(tmp_path):
    files = {"src/jase/x.py": "", "docs/README.md":
             "`src/jase/x.py` `src/jase/falta.py` `src/jase/x.py:12` `src/jase/*.py`"}
    found = problems(tmp_path, files)
    assert len(found) == 1 and "src/jase/falta.py" in found[0]


def test_backticks_outside_repo_roots_are_ignored(tmp_path):
    files = {"src/a.py": "", "docs/README.md":
             "`code/client/cl_main.cpp` `System\\UT.exe` `jase://x` `<ruta>/a.py` "
             "`src/<familia>.py` `tests/fixtures/<juego>_*.bin`"}
    assert problems(tmp_path, files) == []


def test_personal_data_in_docs(tmp_path):
    text = ("C:\\Users\\ana\\juego\n/home/ana/x\n/mnt/c/Users/ana\n8.8.4.4\n"
            "clave 0123456789abcdef0123456789ABCDEF\n")
    found = problems(tmp_path, {"docs/x.md": text})
    assert len(found) == 5


def test_documentation_ranges_and_placeholders_are_allowed(tmp_path):
    text = ("192.0.2.1 198.51.100.7 203.0.113.9 127.0.0.1 0.0.0.0 255.255.255.255\n"
            "<CARPETA_DEL_JUEGO>\\System `%APPDATA%\\JimmysASE`\n")
    assert problems(tmp_path, {"docs/x.md": text}) == []


def test_readme_is_not_checked_for_personal_data(tmp_path):
    assert problems(tmp_path, {"README.md": "master 198.18.0.1 y 8.8.4.4"}) == []


def test_local_plans_are_excluded(tmp_path):
    assert problems(tmp_path, {"docs/superpowers/p.md": "[x](falta.md) /home/ana"}) == []


def test_memory_stage_with_more_than_three_bullets(tmp_path):
    memory = ("# Memoria\n\n## 2026-11-05 — bien\n- a\n- b\n- c\n\n"
              "## 2026-11-04 — mal\n- a\n- b\n- c\n- d\n  - subnivel no cuenta\n")
    found = problems(tmp_path, {"docs/ai/PROJECT_MEMORY.md": memory})
    assert len(found) == 1 and "2026-11-04" in found[0]


def test_memory_stage_heading_format(tmp_path):
    memory = "# Memoria\n\n## ayer — sin fecha\n- a\n"
    found = problems(tmp_path, {"docs/ai/PROJECT_MEMORY.md": memory})
    assert len(found) == 1 and "AAAA-MM-DD" in found[0]


def test_ip_at_end_of_sentence_is_flagged(tmp_path):
    assert len(problems(tmp_path, {"docs/x.md": "Servidor 8.8.4.4."})) == 1


def test_titled_and_reference_links_are_checked(tmp_path):
    text = '[x](falta.md "título")\n\n[r]: otra-falta.md\n'
    found = problems(tmp_path, {"docs/README.md": text})
    assert len(found) == 2


def test_forward_slash_drive_path_is_personal(tmp_path):
    assert len(problems(tmp_path, {"docs/x.md": "C:/Users/ana/juego"})) == 1


def test_module_paths_relative_to_package_are_checked(tmp_path):
    files = {"src/jase/ui/x.py": "", "docs/README.md": "`ui/x.py` `ui/falta.py`"}
    found = problems(tmp_path, files)
    assert len(found) == 1 and "ui/falta.py" in found[0]


def test_star_bullets_count_in_memory(tmp_path):
    memory = "# M\n\n## 2026-11-04 — t\n* a\n* b\n- c\n* d\n"
    assert len(problems(tmp_path, {"docs/ai/PROJECT_MEMORY.md": memory})) == 1


def test_only_tracked_files_count_in_a_git_repo(tmp_path):
    import subprocess
    repo(tmp_path, {"src/a.py": "", "dist/JASE/x.txt": "",
                    "docs/README.md": "`src/a.py` `dist/JASE/x.txt`"})
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    subprocess.run(["git", "add", "src", "docs"], cwd=tmp_path, check=True)
    found = check_docs.check(tmp_path)
    assert len(found) == 1 and "dist/JASE/x.txt" in found[0]

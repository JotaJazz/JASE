//! Archivos del empaquetado (fase 6): lo que el paquete promete se lee del repo.

use std::path::Path;

fn repo(path: &str) -> String {
    let full = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display()))
}

#[test]
fn about_accepts_no_copyleft() {
    let about = repo("rust/about.toml");
    let start = about.find("accepted = [").expect("accepted") + "accepted = [".len();
    let end = start + about[start..].find(']').expect("cierre de accepted");
    let accepted: Vec<&str> = about[start..end]
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .flat_map(|l| l.split(','))
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .collect();
    assert!(!accepted.is_empty());
    for lic in &accepted {
        for bad in ["GPL", "EUPL", "SSPL"] {
            assert!(!lic.contains(bad), "licencia copyleft aceptada: {lic}");
        }
    }
    assert!(about.contains("x86_64-pc-windows-msvc"));
    assert!(about.contains("x86_64-unknown-linux-gnu"));
    assert!(about.contains(r#"workarounds = ["ring"]"#));
}

#[test]
fn about_template_credits_iced_and_vendored_code() {
    let hbs = repo("rust/about.hbs");
    for needle in ["Iced", "Timothy Landers", "huffman.rs", "{{{"] {
        assert!(hbs.contains(needle), "falta {needle}");
    }
    assert!(!hbs.contains("PySide6"));
}

#[test]
fn workspace_crates_are_not_published() {
    for krate in ["jase", "jase-core", "jase-cli"] {
        let toml = repo(&format!("rust/{krate}/Cargo.toml"));
        assert!(
            toml.contains("publish = false"),
            "{krate} sin publish = false"
        );
    }
}

/// El bloque de nivel superior `key:` de un YAML (hasta la siguiente clave sin sangría).
fn yaml_block<'a>(yaml: &'a str, key: &str) -> &'a str {
    let head = format!("\n{key}:\n");
    let rest = &yaml[yaml.find(&head).unwrap_or_else(|| panic!("falta {key}:")) + head.len()..];
    let end = rest
        .match_indices('\n')
        .map(|(i, _)| i + 1)
        .find(|&i| rest[i..].starts_with(|c: char| c.is_ascii_alphabetic()))
        .unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn package_workflow_never_publishes() {
    let wf = repo(".github/workflows/package.yml").replace("\r\n", "\n");
    assert!(wf.contains("\npermissions:\n  contents: read\n"));
    for bad in [
        "contents: write",
        "tags:",
        "softprops/",
        "gh release",
        "secrets.",
    ] {
        assert!(!wf.contains(bad), "package.yml contiene {bad}");
    }
    let on = yaml_block(&wf, "on");
    assert!(on.contains("branches: [master, rust]"), "{on}");
    assert!(on.contains("workflow_dispatch:"), "{on}");
    // release.yml la llama: la versión sale del job `linux`.
    assert!(on.contains("\n  workflow_call:\n"), "{on}");
    assert!(
        on.contains("value: ${{ jobs.linux.outputs.version }}"),
        "{on}"
    );
    assert!(!on.contains("release:"), "{on}");
    // Las acciones fijadas por SHA: `workflow_actions_pinned_by_sha`.
}

/// Las líneas que fijan appimagetool y el runtime (URLs y SHA-256).
fn appimage_pins(yaml: &str) -> Vec<String> {
    yaml.lines()
        .map(str::trim)
        .filter(|l| {
            l.starts_with("curl -fsSLo ")
                || l.ends_with("  appimagetool")
                || l.ends_with("  runtime-x86_64")
        })
        .map(str::to_owned)
        .collect()
}

#[test]
fn package_workflow_names_match_release() {
    let wf = repo(".github/workflows/package.yml").replace("\r\n", "\n");
    for needle in [
        "JASE-${{ steps.ver.outputs.version }}-x86_64.AppImage",
        "name: JASE-${{ steps.ver.outputs.version }}-linux-x86_64",
        "runs-on: ubuntu-22.04",
    ] {
        assert!(wf.contains(needle), "falta {needle}");
    }
    let pins = appimage_pins(&wf);
    assert_eq!(pins.len(), 4, "{pins:?}");
    // Los pines viven solo en package.yml: release.yml la reusa.
    let release = repo(".github/workflows/release.yml");
    assert_eq!(appimage_pins(&release), Vec::<String>::new());
}

#[test]
fn linux_script_ships_notices_not_pyside() {
    let sh = repo("scripts/package_linux.sh");
    for needle in [
        "set -euo pipefail",
        "JASE-LICENSE.txt",
        "DB-IP.txt",
        "flag-icons-MIT.txt",
        "THIRD-PARTY.txt",
        "scripts/build_appimage.sh",
        "--locked",
        r#"cd "$(dirname "$0")/..""#,
    ] {
        assert!(sh.contains(needle), "falta {needle}");
    }
    assert!(!sh.contains("PySide6"));
}

#[test]
fn zip_keeps_internal_for_1_3_0_updater() {
    let readme = repo("packaging/windows/internal-README.txt");
    assert!(!readme.trim().is_empty());
    assert!(readme.is_ascii());
    let ps = repo("scripts/package_windows.ps1");
    for needle in [
        "internal-README.txt",
        r"_internal\README.txt",
        r"Compress-Archive -Path dist\JASE",
        "JASE.exe",
    ] {
        assert!(ps.contains(needle), "falta {needle}");
    }
    // El zip y el actualizador hablan de lo mismo.
    assert_eq!(jase_core::portable_update::ZIP_TOP, "JASE");
    assert_eq!(jase_core::portable_update::EXE_NAME, "JASE.exe");
}

#[test]
fn windows_script_is_ascii_and_strict() {
    let ps = repo("scripts/package_windows.ps1");
    assert!(ps.is_ascii(), "PowerShell 5 lee sin BOM como ANSI");
    for needle in [
        "$ErrorActionPreference = 'Stop'",
        "--locked",
        r"packaging\jase.iss",
        "-RedirectStandardOutput",
        "ICED_BACKEND",
        "JASE smoke OK",
        "JASE-LICENSE.txt",
        "DB-IP.txt",
        "flag-icons-MIT.txt",
        "THIRD-PARTY.txt",
        // Un cuelgue del smoke no debe gastar el plazo entero del job.
        "WaitForExit(",
        "$PSScriptRoot",
    ] {
        assert!(ps.contains(needle), "falta {needle}");
    }
    assert!(!ps.contains("PySide6"));
}

#[test]
fn package_workflow_windows_job() {
    let wf = repo(".github/workflows/package.yml").replace("\r\n", "\n");
    let release = repo(".github/workflows/release.yml").replace("\r\n", "\n");
    for needle in [
        "runs-on: windows-latest",
        "choco install innosetup --version 6.7.1",
        "name: JASE-${{ steps.ver.outputs.version }}-win64",
        "            JASE-${{ steps.ver.outputs.version }}-win64.zip\n            JASE-${{ steps.ver.outputs.version }}-setup.exe\n",
    ] {
        assert!(wf.contains(needle), "falta {needle}");
        assert!(!release.contains(needle), "release.yml copia {needle}");
    }
}

/// La única cita del árbol de Python: el actualizador de la tag `v1.3.0` (D9). Partida en
/// dos para que este archivo no la contenga.
const PORTABLE_UPDATE_130: &str = concat!("git show v1.3.0:src/", "jase/portable_update.py");

#[test]
fn no_reference_to_python_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let out = std::process::Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--",
            "rust",
            "scripts",
            "packaging",
            ".github",
        ])
        .current_dir(&root)
        .output()
        .expect("git ls-files");
    assert!(out.status.success(), "git ls-files falló");
    let needles = [
        // Con el separador final: `rust/jase-core/src/jase_url.rs` es legítimo.
        concat!("src/", "jase/"),
        concat!("src\\", "jase\\"),
        concat!(".", "venv"),
    ];
    let mut found = Vec::new();
    for name in String::from_utf8(out.stdout)
        .expect("utf-8")
        .split_terminator('\0')
    {
        if name.starts_with("rust/target/") {
            continue;
        }
        let bytes = std::fs::read(root.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let text = String::from_utf8_lossy(&bytes);
        for (i, line) in text.lines().enumerate() {
            let allowed =
                name == "scripts/test_upgrade_windows.ps1" && line.contains(PORTABLE_UPDATE_130);
            if !allowed && needles.iter().any(|n| line.contains(n)) {
                found.push(format!("{name}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn upgrade_script_uses_real_1_3_0() {
    let ps = repo("scripts/test_upgrade_windows.ps1");
    assert!(ps.is_ascii(), "PowerShell 5 lee sin BOM como ANSI");
    for needle in [
        "releases/download/v1.3.0/JASE-1.3.0-setup.exe",
        "releases/download/v1.3.0/JASE-1.3.0-win64.zip",
        // Los `digest` de la release v1.3.0 (D9).
        "48029af828b5f60b6fc1bc8d8f59917808bc43e133d6a832336a4bedf50f61ec",
        "5314b3615f27d307e7c6794ad9b4c32c92f3d5d6f9b143b1ba30d4baca57db80",
        "Get-FileHash",
        PORTABLE_UPDATE_130,
        "python -I",
        "extract_release",
        "write_updater",
        "/VERYSILENT",
        "/CURRENTUSER",
        "unins000.exe",
        "{5A639811-DD43-4C26-AF69-0EFDE699CB93}_is1",
        "-Ci",
        "$ErrorActionPreference = 'Stop'",
        "WaitForExit(",
        // Si una comprobación falla, el finally limpia lo que dejó esta corrida.
        "} finally {",
        "Stop-Process",
        "$script:Installed = $true",
        "HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        // D10: el instalador sobre 1.3.0 no deja los avisos viejos de licenses\.
        "foreach ($old in 'PySide6-LGPL-3.0.txt', 'CPython-LICENSE.txt') {",
        "throw \"Quedo el aviso viejo licenses\\$old\"",
    ] {
        assert!(ps.contains(needle), "falta {needle}");
    }
    let fin = &ps[ps.find("} finally {").expect("finally")..];
    for needle in ["Stop-Process", "$script:Installed", "unins000.exe"] {
        assert!(fin.contains(needle), "el finally no tiene {needle}");
    }
    let wf = repo(".github/workflows/package.yml").replace("\r\n", "\n");
    assert!(wf.contains("refs/tags/v1.3.0"));
    let run = wf
        .lines()
        .find(|l| l.contains("test_upgrade_windows.ps1 -Version"))
        .expect("falta el paso de actualización");
    assert!(run.contains("-Ci"), "{run}");
    let step = wf
        .find("test_upgrade_windows.ps1 -Version")
        .expect("falta el paso de actualización");
    let upload = wf.find("actions/upload-artifact").expect("upload-artifact");
    assert!(step < upload, "la prueba va antes de subir los artifacts");
}

/// Las líneas de una sección `[name]` del `.iss`, sin vacías ni comentarios.
fn iss_section(iss: &str, name: &str) -> Vec<String> {
    let head = format!("[{name}]");
    iss.lines()
        .skip_while(|l| l.trim() != head)
        .skip(1)
        .take_while(|l| !l.starts_with('['))
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with(';'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn iss_fixed_app_id() {
    // Las llaves dobles escapan la llave inicial en Inno: AppId={{GUID}.
    let iss = repo("packaging/jase.iss");
    assert!(
        iss.lines()
            .any(|l| l.trim_end() == "AppId={{5A639811-DD43-4C26-AF69-0EFDE699CB93}"),
        "AppId cambió: Windows vería otra app"
    );
}

#[test]
fn iss_never_deletes_user_data() {
    let iss = repo("packaging/jase.iss");
    for name in ["InstallDelete", "UninstallDelete"] {
        for line in iss_section(&iss, name) {
            for bad in ["JimmysASE", "{userappdata}", "{appdata}"] {
                assert!(
                    !line.contains(bad),
                    "[{name}] borra datos del usuario: {line}"
                );
            }
        }
    }
    assert_eq!(iss_section(&iss, "UninstallDelete"), Vec::<String>::new());
}

#[test]
fn iss_install_delete_only_stale_folders() {
    // D10: solo los restos de 1.3.0 (PyInstaller y sus avisos de licencias), nunca {app} entero.
    let iss = repo("packaging/jase.iss");
    assert_eq!(
        iss_section(&iss, "InstallDelete"),
        [
            r#"Type: filesandordirs; Name: "{app}\_internal""#,
            r#"Type: filesandordirs; Name: "{app}\licenses""#,
        ]
    );
}

#[test]
fn iss_registers_jase_protocol() {
    let iss = repo("packaging/jase.iss");
    assert_eq!(
        iss_section(&iss, "Registry"),
        [
            r#"Root: HKA; Subkey: "Software\Classes\jase"; ValueType: string; ValueName: ""; ValueData: "URL:JASE"; Flags: uninsdeletekey"#,
            r#"Root: HKA; Subkey: "Software\Classes\jase"; ValueType: string; ValueName: "URL Protocol"; ValueData: """#,
            r#"Root: HKA; Subkey: "Software\Classes\jase\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\JASE.exe,0""#,
            r#"Root: HKA; Subkey: "Software\Classes\jase\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\JASE.exe"" ""%1""""#,
        ]
    );
}

#[test]
fn workflow_actions_pinned_by_sha() {
    let name_ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
    };
    for file in ["ci.yml", "package.yml", "release.yml"] {
        let wf = repo(&format!(".github/workflows/{file}")).replace("\r\n", "\n");
        for line in wf.lines() {
            let t = line.trim_start();
            let Some(at) = t
                .strip_prefix("- uses:")
                .or_else(|| t.strip_prefix("uses:"))
            else {
                continue;
            };
            let pin = at.split_whitespace().next().unwrap_or_default();
            if pin.starts_with("./") {
                continue;
            }
            let (path, sha) = pin.split_once('@').unwrap_or_default();
            let parts: Vec<&str> = path.split('/').collect();
            assert!(
                parts.len() >= 2
                    && parts.iter().all(|p| name_ok(p))
                    && sha.len() == 40
                    && sha
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "{file}: acción sin SHA fijo: {line}"
            );
        }
        assert_eq!(
            wf.matches("actions/checkout@").count(),
            wf.matches("persist-credentials: false").count(),
            "{file}: checkout que guarda credenciales"
        );
        let jobs = yaml_block(&wf, "jobs");
        let mut starts: Vec<usize> = jobs
            .match_indices('\n')
            .map(|(i, _)| i + 1)
            .chain([0])
            .filter(|&i| {
                jobs[i..].starts_with("  ")
                    && jobs[i + 2..].starts_with(|c: char| c.is_ascii_alphanumeric())
            })
            .collect();
        starts.sort_unstable();
        assert!(!starts.is_empty(), "{file}: sin jobs");
        for (n, &start) in starts.iter().enumerate() {
            let job = &jobs[start..starts.get(n + 1).copied().unwrap_or(jobs.len())];
            // Un job que llama a un workflow reusable no admite `timeout-minutes`.
            let reusable = job.contains("\n    uses: ");
            assert!(
                reusable || job.contains("\n    timeout-minutes: "),
                "{file}: job sin timeout-minutes: {}",
                job.lines().next().unwrap_or_default()
            );
        }
    }
}

#[test]
fn iss_directives() {
    let iss = repo("packaging/jase.iss");
    // ISCC lee los .iss sin BOM como ANSI: solo ASCII evita depender de la codificación.
    assert!(iss.is_ascii());
    let setup = iss_section(&iss, "Setup");
    for line in [
        r"DefaultDirName={autopf}\JASE",
        "PrivilegesRequired=lowest",
        "PrivilegesRequiredOverridesAllowed=dialog",
        "ArchitecturesAllowed=x64compatible",
        "ArchitecturesInstallIn64BitMode=x64compatible",
    ] {
        assert!(setup.iter().any(|l| l.trim_end() == line), "falta {line}");
    }
    assert_eq!(
        iss_section(&iss, "Languages"),
        [
            r#"Name: "english"; MessagesFile: "compiler:Default.isl""#,
            r#"Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl""#,
            r#"Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl""#,
            r#"Name: "french"; MessagesFile: "compiler:Languages\French.isl""#,
            r#"Name: "german"; MessagesFile: "compiler:Languages\German.isl""#,
            r#"Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl""#,
        ]
    );
    // El acceso de escritorio, desmarcado por defecto.
    assert_eq!(
        iss_section(&iss, "Tasks"),
        [
            r#"Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked"#
        ]
    );
    assert_eq!(
        iss_section(&iss, "Icons"),
        [
            r#"Name: "{autoprograms}\JASE"; Filename: "{app}\JASE.exe""#,
            r#"Name: "{autodesktop}\JASE"; Filename: "{app}\JASE.exe"; Tasks: desktopicon"#,
        ]
    );
    assert_eq!(
        iss_section(&iss, "Run"),
        [
            r#"Filename: "{app}\JASE.exe"; Description: "{cm:LaunchProgram,JASE}"; Flags: nowait postinstall skipifsilent"#
        ]
    );
    // La carpeta de datos solo se nombra en comentarios (el motivo de no borrarla).
    for line in iss.lines().filter(|l| !l.trim_start().starts_with(';')) {
        assert!(!line.contains("JimmysASE"), "{line}");
    }
}

#[test]
fn appimage_script_and_desktop_file() {
    let sh = repo("scripts/build_appimage.sh");
    assert!(sh.contains("set -euo pipefail"));
    // Sin runtime "continuous" descargado al vuelo.
    assert!(sh.contains("--runtime-file"));
    let desktop = repo("packaging/linux/jase.desktop");
    for line in [
        "Exec=JASE %u",
        "Icon=jase",
        "MimeType=x-scheme-handler/jase;",
    ] {
        assert!(
            desktop.lines().any(|l| l.trim_end() == line),
            "falta {line}"
        );
    }
}

/// El workflow sin `\r`, con `permissions: contents: read` arriba.
fn workflow_read_only(file: &str) -> String {
    let wf = repo(&format!(".github/workflows/{file}")).replace("\r\n", "\n");
    assert_eq!(
        yaml_block(&wf, "permissions").trim_end(),
        "  contents: read",
        "{file}: permisos de nivel superior"
    );
    wf
}

#[test]
fn ci_triggers() {
    let wf = workflow_read_only("ci.yml");
    let on = yaml_block(&wf, "on");
    assert!(
        on.starts_with("  push:\n    branches:\n      - \"**\"\n"),
        "{on}"
    );
    assert!(on.contains("\n  pull_request:\n"), "{on}");
    assert!(!wf.contains("tags-ignore"));
}

#[test]
fn ci_has_no_python() {
    let wf = repo(".github/workflows/ci.yml");
    for bad in ["setup-python", "pip", "pytest", "ruff", "mypy"] {
        assert!(!wf.contains(bad), "ci.yml todavía nombra {bad}");
    }
}

#[test]
fn release_triggers_on_version_tags() {
    let wf = workflow_read_only("release.yml");
    let on = yaml_block(&wf, "on");
    assert!(
        on.starts_with("  push:\n    tags:\n      - \"v*\"\n"),
        "{on}"
    );
}

#[test]
fn dependabot_updates_actions_weekly() {
    let yml = repo(".github/dependabot.yml").replace("\r\n", "\n");
    assert!(yml.starts_with("version: 2\n"));
    assert_eq!(yml.matches("package-ecosystem:").count(), 1);
    for needle in [
        r#"package-ecosystem: "github-actions""#,
        r#"directory: "/""#,
        r#"interval: "weekly""#,
    ] {
        assert!(yml.contains(needle), "falta {needle}");
    }
}

/// El job `name` del bloque `jobs:` (hasta el siguiente job).
fn job_block<'a>(wf: &'a str, name: &str) -> &'a str {
    let jobs = yaml_block(wf, "jobs");
    let head = format!("  {name}:\n");
    let start = if jobs.starts_with(&head) {
        0
    } else {
        jobs.find(&format!("\n{head}"))
            .unwrap_or_else(|| panic!("falta el job {name}"))
            + 1
    };
    let rest = &jobs[start + head.len()..];
    let end = rest
        .match_indices("\n  ")
        .map(|(i, _)| i + 1)
        .find(|&i| rest[i + 2..].starts_with(|c: char| c.is_ascii_alphanumeric()))
        .unwrap_or(rest.len());
    &jobs[start..start + head.len() + end]
}

#[test]
fn release_reuses_package_and_is_draft_prerelease() {
    let wf = workflow_read_only("release.yml");
    for needle in [
        "uses: ./.github/workflows/package.yml",
        "needs: package",
        "if: startsWith(github.ref, 'refs/tags/v')",
        "pattern: JASE-${{ needs.package.outputs.version }}-*",
        "merge-multiple: true",
        "draft: true",
        "prerelease: true",
        "name: JASE ${{ needs.package.outputs.version }}",
        "body_path: packaging/release-notes.md",
        "fail_on_unmatched_files: true",
        "JASE-${{ needs.package.outputs.version }}-setup.exe",
        "JASE-${{ needs.package.outputs.version }}-win64.zip",
        "JASE-${{ needs.package.outputs.version }}-x86_64.AppImage",
        "workflow_dispatch:",
    ] {
        assert!(wf.contains(needle), "falta {needle}");
    }
    for bad in ["setup-python", "pyinstaller", "pip install", "jase.spec"] {
        assert!(!wf.to_lowercase().contains(bad), "release.yml nombra {bad}");
    }
    let release = job_block(&wf, "release");
    assert!(release.contains("\n    permissions:\n      contents: write\n"));
    // Relanzar sobre una release ya publicada la volvería borrador con otros assets.
    let guard = release
        .find(r#"gh api "repos/$GITHUB_REPOSITORY/releases/tags/$TAG" >/dev/null 2>&1"#)
        .expect("falta el paso que frena una release ya publicada");
    assert!(release[guard..].contains("exit 1"));
    assert!(release.contains("GH_TOKEN: ${{ github.token }}"));
    assert!(release.contains("TAG: ${{ github.ref_name }}"));
    let download = release.find("actions/download-artifact").expect("download");
    assert!(guard < download, "el freno va antes de bajar los paquetes");
    assert_eq!(wf.matches("contents: write").count(), 1, "{wf}");
    assert!(!job_block(&wf, "package").contains("permissions"));
}

#[test]
fn package_checks_tag_matches_version() {
    let wf = repo(".github/workflows/package.yml").replace("\r\n", "\n");
    for name in ["windows", "linux"] {
        let job = job_block(&wf, name);
        assert!(
            job.contains("\n    outputs:\n      version: ${{ steps.ver.outputs.version }}\n"),
            "{name}: sin outputs.version"
        );
        let read = job.find("name: Read version").expect("Read version");
        let check = &job[read..];
        let at = check
            .find("if: github.ref_type == 'tag'")
            .unwrap_or_else(|| panic!("{name}: sin el paso del tag"));
        assert!(
            check[at..].contains(r#"[ "$GITHUB_REF_NAME" != "v$VERSION" ]"#),
            "{name}: el paso no compara el tag"
        );
        assert!(check[at..].contains("exit 1"), "{name}");
        let upload = check.find("actions/upload-artifact").expect("upload");
        assert!(at < upload, "{name}: el tag se comprueba antes de subir");
    }
}

#[test]
fn release_notes_match_version() {
    let notes = repo("packaging/release-notes.md");
    let (en, es) = notes.split_once("<!-- ES -->").expect("falta <!-- ES -->");
    let v = env!("CARGO_PKG_VERSION");
    for half in [en, es] {
        assert!(half.contains(&format!("# JASE {v}")), "falta el título");
        for suffix in ["setup.exe", "win64.zip", "x86_64.AppImage"] {
            let file = format!("JASE-{v}-{suffix}");
            assert!(half.contains(&file), "falta {file}");
        }
        // D3: el hash lo da el `digest` de la API, no el cuerpo.
        assert!(!half.to_lowercase().contains("sha256"));
        assert!(!half.contains("PySide6"));
    }
}

# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec de JASE (onedir). Uso: pyinstaller jase.spec
# Los datos se resuelven relativos a este archivo (SPECPATH).

import os
import sys

from PyInstaller.utils.hooks import copy_metadata

WINDOWS = sys.platform == "win32"

EXCLUDES = [
    "PySide6.QtWebEngineCore", "PySide6.QtWebEngineWidgets", "PySide6.QtWebEngineQuick",
    "PySide6.QtWebChannel", "PySide6.QtWebSockets", "PySide6.QtWebView",
    "PySide6.Qt3DCore", "PySide6.Qt3DRender", "PySide6.Qt3DInput", "PySide6.Qt3DLogic",
    "PySide6.Qt3DAnimation", "PySide6.Qt3DExtras",
    "PySide6.QtMultimedia", "PySide6.QtMultimediaWidgets",
    "PySide6.QtQml", "PySide6.QtQuick", "PySide6.QtQuickWidgets", "PySide6.QtQuick3D",
    "PySide6.QtQuickControls2", "PySide6.QtDesigner", "PySide6.QtCharts",
    "PySide6.QtDataVisualization", "PySide6.QtBluetooth", "PySide6.QtNfc",
    "PySide6.QtSensors", "PySide6.QtSerialPort", "PySide6.QtPdf", "PySide6.QtPdfWidgets",
    "PySide6.QtPositioning", "PySide6.QtLocation", "PySide6.QtRemoteObjects",
    "PySide6.QtScxml", "PySide6.QtSql", "PySide6.QtTest", "PySide6.QtHelp",
    "PySide6.QtOpenGL", "PySide6.QtOpenGLWidgets", "PySide6.QtNetworkAuth",
    "PySide6.QtSpatialAudio", "PySide6.QtGraphs", "PySide6.QtTextToSpeech",
    "tkinter", "pytest",
]

# Linux: plugins de Qt que la app no usa y que arrastran Qt Quick/QML, Qt Pdf o todo GTK.
# libstdc++ y libgcc_s quedan fuera para usar las del sistema: unas más viejas que las que
# piden los drivers de Mesa del usuario impedirían cargarlos.
LINUX_DROP = (
    "libqgtk3.so", "libqtvirtualkeyboardplugin.so", "libqpdf.so",
    "libQt6Quick", "libQt6Qml", "libQt6Pdf", "libQt6VirtualKeyboard",
    "libgtk-3.so", "libgdk-3.so", "libgdk_pixbuf-2.0.so", "libglycin-", "libatk-",
    "libatspi.so", "libcairo", "libpango", "libepoxy.so", "libthai.so", "libdatrie.so",
    "libpixman-1.so", "libseccomp.so", "libstdc++.so", "libgcc_s.so",
)

a = Analysis(
    ["src/jase/__main__.py"],
    pathex=["src"],
    binaries=[],
    # copy_metadata: dist-info de jase, para que importlib.metadata dé la versión.
    datas=[("src/jase/assets", "jase/assets")] + copy_metadata("jase"),
    # Linux: Secret Service (GNOME Keyring, KWallet) vía secretstorage.
    hiddenimports=[
        "keyring.backends.Windows" if WINDOWS else "keyring.backends.SecretService",
        "keyring.backends.fail",
        "PySide6.QtSvg",
    ],
    hookspath=[],
    runtime_hooks=[],
    excludes=EXCLUDES,
    noarchive=False,
)
if not WINDOWS:
    a.binaries = [b for b in a.binaries if not os.path.basename(b[0]).startswith(LINUX_DROP)]
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="JASE",
    debug=False,
    strip=False,
    upx=False,
    console=False,
    # En Linux el ejecutable no lleva icono: lo pone el .desktop del AppImage.
    icon="src/jase/assets/app_icon.ico" if WINDOWS else None,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    name="JASE",
)

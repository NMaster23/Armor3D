from pathlib import Path
import importlib.util

from PyInstaller.building.build_main import Analysis, PYZ, EXE


ROOT = Path(SPEC).resolve().parent
UI_DIR = ROOT / "CTK UI"
ASSETS_DIR = ROOT / "Assets"

extension_spec = importlib.util.find_spec("armor_core.armor_core")
if extension_spec is None or extension_spec.origin is None:
    raise RuntimeError(
        "The armor_core Python extension is not installed. "
        "Build and install it before running PyInstaller."
    )


a = Analysis(
    [str(UI_DIR / "ui.py")],
    pathex=[str(ROOT), str(UI_DIR)],
    # Put the compiled PyO3 module inside the armor_core package directory.
    binaries=[(extension_spec.origin, "armor_core")],
    datas=[(str(ASSETS_DIR), "Assets")],
    hiddenimports=["armor_core", "armor_core.armor_core"],
    excludes=[],
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name="Armor3D",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=False,
)

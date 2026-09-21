from pathlib import Path
import importlib.util

from PyInstaller.building.build_main import Analysis, PYZ, EXE


ROOT = Path(SPEC).resolve().parent
UI_DIR = ROOT / "CTK UI"
ASSETS_DIR = ROOT / "Assets"

module_spec = importlib.util.find_spec("armor_core")
if module_spec is None or module_spec.origin is None:
    raise RuntimeError(
        "The armor_core Python extension is not installed. "
        "Build and install it before running PyInstaller."
    )


a = Analysis(
    [str(UI_DIR / "ui.py")],
    pathex=[str(ROOT), str(UI_DIR)],
    binaries=[(module_spec.origin, ".")],
    datas=[(str(ASSETS_DIR), "Assets")],
    hiddenimports=["armor_core"],
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

from pathlib import Path
import importlib.util

from PyInstaller.building.build_main import Analysis, PYZ, EXE
from PyInstaller.utils.hooks import collect_all

def collect_optional(package):
    if importlib.util.find_spec(package) is None:
        return [], [], []
    return collect_all(package)

trimesh_datas, trimesh_binaries, trimesh_hiddenimports = collect_all("trimesh")
cascadio_datas, cascadio_binaries, cascadio_hiddenimports = collect_all("cascadio")
lxml_datas, lxml_binaries, lxml_hiddenimports = collect_optional("lxml")
networkx_datas, networkx_binaries, networkx_hiddenimports = collect_optional("networkx")

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
    binaries=[
        (extension_spec.origin, "armor_core"),
        *trimesh_binaries,
        *cascadio_binaries,
        *lxml_binaries,
        *networkx_binaries,
    ],
    datas=[
        (str(ASSETS_DIR), "Assets"),
        *trimesh_datas,
        *cascadio_datas,
        *lxml_datas,
        *networkx_datas,
    ],
    hiddenimports=[
        "armor_core",
        "armor_core.armor_core",
        *trimesh_hiddenimports,
        *cascadio_hiddenimports,
        *lxml_hiddenimports,
        *networkx_hiddenimports,
    ],
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

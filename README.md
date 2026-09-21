# Armor3D
### Armor3D is an attempt at re-making Rhinoceros 8, a CAD drawing software. Armor3D is built with Python (main UI with CTK) and Rust (grid drawing + all 3D calculations).

## Rust Side

The rust side utilizes wgpu for closer system level access, but is harder to code in. It requires much more boilerplate but currently a world graph is implemented on x, y, and z. There is a working 3D camera. Drawing also works but currently it is hardcoded.

## Week 1 progress
- Main workspace with a command input and history
- Left toolbar with hover effects and tooltips
- AI sidebar with a typing animation
- Visual toggles for Grid Snap, Ortho, and Osnap
- HARVEST THEMED UI FOR WEEK 1 

## Current limitations
Week 1 - this release only features drawing polylines on a grid, with many of the other features not available at the time. The main UI and most of the buttons, however, have been completed. 

## Run locally
This version is specifically built for Windows.

The UI imports the compiled `armor_core` Python extension, so build and install it before running the app. In PowerShell, from the project root:

```powershell
$py = "$env:LOCALAPPDATA\Python\pythoncore-3.14-64\python.exe"
$env:Path = "$HOME\.cargo\bin;$env:Path"
& $py -m pip install customtkinter pillow maturin
& $py -m maturin build --release --manifest-path "armor-core\Cargo.toml" --interpreter $py
$wheel = Get-ChildItem "armor-core\target\wheels\armor_core-*.whl" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
& $py -m pip install --force-reinstall $wheel.FullName
& $py "CTK UI\ui.py"
```

Use the path to your own `python.exe` if it differs. After the wheel is installed, only the last command is needed to run the app. Rebuild the wheel after changing Rust code.

## Viewport navigation
With the pointer over the grid, use the mouse wheel to zoom, right-drag to pan, or Shift + right-drag to orbit. Click the grid to focus it, then use W/A/S/D to move and the arrow keys to rotate the view. Press 2 to toggle a top-down orthographic 2D view; press 2 again to restore the previous 3D view. Left-click still draws points.

Keep the Assets folder in the project root so the fonts and icons can load.

## Next Steps
### Week 1
- Start AI integration
- Complete all button functions
- Improve on 3D drawing

## Week 1 theme
We've followed this week's "harvest" theme by basing our main UI around it, using colors such as orange and green. 

## Where AI assistance has been used
- Creating some of the UI animations
- Fix/find bugs
- Ideas/inspiration
- Rest of AI usage that has not been mentioned here is most likely in the commits

## DEMO



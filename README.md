# Armor3D
### Armor3D is an attempt at re-making Rhinoceros 8, a CAD drawing software. Armor3D is built with Python (main UI with CTK) and Rust (grid drawing + all 3D calculations).

## NOTICE
Armor3D is a work in progress. Some tools are incomplete, and bugs are expected. Feedback and bug reports are appreciated!

## Rust Side
The Rust core uses wgpu for viewport rendering and geometry, and is exposed to the Python interface through PyO3. The viewport handles drawing, snapping, selection, and editing operations. Some newer tools and rendering work are still being developed.

## Week 1 Progress
- Main workspace with a command input and history
- Left toolbar with hover effects and tooltips
- AI sidebar with a typing animation
- Visual toggles for Grid Snap, Ortho, and Osnap
- HARVEST THEMED UI FOR WEEK 1 

## Week 2 Progress
- AI chat - only chatbot for now with ability to switch between models
- Polyline drawing - w/ osnap, delete, drag, and the ability to move
- Save drawing feature added.
- Command line typing commands added

## Week 3 Progress
- Added save and importing DXF drawings
- Added two-click circle creation using Rust geometry.
- Added mirroring, selected-object duplication, and arrow-key movement.
- Added undo and redo, including undoing the most recent segment while drawing a polyline.
- Improved grid snapping with adjustable spacing and added a live coordinate readout.
- Added DXF import and export, supporting LINE and LWPOLYLINE entities.
- Added New and Save workflows, file names in the window title, and JSON-backed settings.
- Improved coordinate awareness and response behavior in the AI sidebar.
- Added text-rendering support and began work on curve drawing and a textured renderer.
- Added ability to copy objects
- Control Z and Control Y capabilities

## Current limitations
Armor3D does not yet include the full feature set of Rhino. Curve drawing and the newer textured-renderer work are in progress, and some tools may be incomplete. DXF import currently handles LINE and LWPOLYLINE entities.

## Download
Downlod the EXE from the latest release: (https://github.com/NMaster23/Armor3D/releases/tag/v3.1). After that is completed, ignore the Windows security feature, and then you're in the app!

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

Use the path to your own `python.exe` if it differs. After the wheel is installed, only the last command is needed to run the app. Rebuild the wheel after changing Rust code. Keep the Assets folder in the project root so the fonts and icons can load.

## CONTROLS
- Right click to pan
- Scroll wheel to zoom
- Type your command to execute - command line
- Press the key "]" to toggle between a 2D and 3D view
- Shift + right drag to orbit
- Press polyline to create shapes and drawings - only drawing ability for week two.
- Use two of the osnap features to snap onto objects - near and end
- Right click to end command - press right click again to run previous command
- Click AI sidebar and enter HACKAI API key at the top right to use the quick chat feature - only chatbot for week 2

## The Project Structure
- `CTK UI/ui.py` - CustomTkinter interface and controls
- `armor-core/` - Rust/wgpu viewport and camera
- `Assets/` - Fonts and toolbar images
- `Armor3D.spec` - Windows executable packaging

## Rust Core and Viewport
The viewport and 3D handling is implemented in Rust through the use of the WGPU crate in rust. The Rust renderer is passed to the frontend using a python extension through PyO3 and was compiled via maturin.
The Rust side provides the 2D and 3D grid, camera, polyline drawing and preview, tessellation, end and near snapping, selection, and shape movement. Python forwards viewport input and manages the surrounding controls. More CAD tools are still in progress.

## Next Steps
- Continue curve-drawing and renderer development.
- Expand drawing, editing, and file-import/export tools.
- Improve and complete the existing CAD commands.

## Week 1 theme
Week 1 used a harvest-inspired palette of orange and green.

## Week 2 theme: Treasure
This week we could not implement the theme but next week we will try to include it in our project.

## Where AI assistance has been used
- Creating some of the UI animations
- Fix/find bugs
- Ideas/inspiration
- Merging Rust grid and Python UI - AI was used here because of the time concern
- Merging controls between Python and Rust - and adding binds
- Fixing Rust features
- Rest of AI usage that has not been mentioned here is most likely in the commits

## Credits
- Nishanth Prabhu - UI, design, and app integration
- Nihaal Mysore Bharath - Rust viewport and grid work

## DEMO
<img width="1099" height="728" alt="image" src="https://github.com/user-attachments/assets/83f01f06-dffd-4aa2-ad75-efb1b0462f7f" />

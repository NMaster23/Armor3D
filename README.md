# Armor3D
### Armor3D is an attempt at re-making Rhinoceros 8, a CAD drawing software. Armor3D is built with Python (main UI with CTK) and Rust (grid drawing + all 3D calculations).

## NOTICE
The drawing for the grid is a temporary solution for week one, and will be changed in week two to match actual Rhino commands. Currently, most of the buttons are not functional, and will be working in week two. The only buttons that work at the moment are: **AI Creation button** - actual AI isn't implemented yet, **File button** - dropdown works not actual function, and the **Analyze button** - only dropdown not actual function. 

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

## Download
Downlaod the EXE from the latest release: (https://github.com/NMaster23/Armor3D/releases/tag/v1). After that is completed, ignore the Windows security feature, and then you're in the app!

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
- Press the number 2 to toggle between a 2D and 3D view
- Shift + right drag to orbit
- Left click to draw (temporary for week one - will use actual Rhino commands during week two)

## The Project Structure
- `CTK UI/ui.py` - CustomTkinter interface and controls
- `armor-core/` - Rust/wgpu viewport and camera
- `Assets/` - Fonts and toolbar images
- `Armor3D.spec` - Windows executable packaging

## Next Steps
### Week 2
- Start AI integration
- Complete all button functions
- Improve on 3D drawing

## Week 1 theme
We've followed this week's "harvest" theme by basing our main UI around it, using colors such as orange and green. 

## Where AI assistance has been used
- Creating some of the UI animations
- Fix/find bugs
- Ideas/inspiration
- Merging Rust grid and Python UI - AI was used here because of the time concern
- Rest of AI usage that has not been mentioned here is most likely in the commits

## Credits
- Nishanth Prabhu - UI, design, and app integration
- Nihaal Mysore Bharath - Rust viewport and grid work

## DEMO
<img width="1097" height="734" alt="Screenshot 2026-09-20 201748" src="https://github.com/user-attachments/assets/cb7e91e4-4478-4a00-b9bb-7285fbdbad4e" />



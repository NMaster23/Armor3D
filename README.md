# Armor3D
### Armor3D is an attempt at re-making Rhinoceros 8, a CAD drawing software. Armor3D is built with Python (main UI with CTK) and Rust (grid drawing + all 3D calculations).

## NOTICE
During week 2, we have gotten the Polyline feature to work, osnap, deleting, moving, AI chat, Command line, and more. However, there may be many bugs we are unaware of, and many features we haven't implemented yet, so expect incomplete tools and occasional issues. Feedback and bug reports are appreciated!

## Rust Side
The rust side utilizes wgpu for closer system level access, but is harder to code in. It requires much more boilerplate but currently a world graph is implemented on x, y, and z. There is a working 3D camera. Drawing also works but currently it is hardcoded.

## Week 1 progress
- Main workspace with a command input and history
- Left toolbar with hover effects and tooltips
- AI sidebar with a typing animation
- Visual toggles for Grid Snap, Ortho, and Osnap
- HARVEST THEMED UI FOR WEEK 1 

## Week 2 progress
- AI chat - only chatbot for now with ability to switch between models
- Polyline drawing - w/ osnap, delete, drag, and the ability to move
- Save drawing feature added.
- Command line typing commands added

## Current limitations
Week 2 - This release allows for the ability to draw different figures and shapes using the polyline feauture, but doesn't have all of the main features the actual Rhino includes; we need to add that in the next week.

## Download
Downlaod the EXE from the latest release: (https://github.com/NMaster23/Armor3D/releases/tag/v2). After that is completed, ignore the Windows security feature, and then you're in the app!

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
The Rust side provides the grid for both 2D and 3D. It also provides the camera, drawing through polyline and it's preview, tessellation, end and near object snapping, selection, and moving shapes. Python then forwards viewport input and manages the surrounding controls. Some of the more complex CAD tools are still incomplete, but are nearly done.

## Next Steps
### Week 2
- Add curves
- Fully implement AI control
- Add grid snap featrues
- Add advanced export features
- Add Polyline Selection
- Add Polyline
- Add Edit Mode

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
- Rest of AI usage that has not been mentioned here is most likely in the commits

## Credits
- Nishanth Prabhu - UI, design, and app integration
- Nihaal Mysore Bharath - Rust viewport and grid work

## DEMO
<img width="1087" height="715" alt="image" src="https://github.com/user-attachments/assets/f5baa85c-80d2-49cc-b563-624c7bcad145" />
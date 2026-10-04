# Armor3D
### Armor3D is an attempt at re-making Rhinoceros 8, a CAD drawing software. Armor3D is built with Python (main UI with CTK) and Rust (grid drawing + all 3D calculations).

## NOTICE
During week 3, we have gotten the object movement feature to work, circle creation, Command line improvements, and more. However, there may be many bugs we are unaware of, and many features we haven't implemented yet, so expect incomplete tools and occasional issues. Feedback and bug reports are appreciated!

## Rust Side
The rust side utilizes wgpu for closer system level access, albeit more boilerplate is required. This week the circle was implemented, so was extrusion, object movement, and other minor tweaks. Since this is not a final release again there maybe be many bugs that this team has not caught yet.

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
- Wired Two Click Circle Command
- Circle Uses Rust Geometry
- Fixed Command Cleanup
- Verified Functionality
- Object Movement
- Python Progress:


## Current limitations
Week 3 - This release allows objects to be extruded, for circles to be created, and the ability to move objects around, but doesn't have all of the main features the actual Rhino includes; we need to add that in the next week.

## Download
Downlod the EXE from the latest release: (https://github.com/NMaster23/Armor3D/releases/tag/v3). After that is completed, ignore the Windows security feature, and then you're in the app!

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
<img width="1099" height="728" alt="image" src="https://github.com/user-attachments/assets/83f01f06-dffd-4aa2-ad75-efb1b0462f7f" />

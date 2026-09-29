$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$ArmorCore = Join-Path $Root "armor-core"
$WheelDirectory = Join-Path $ArmorCore "target\wheels"

# 1. Kill any background processes locking Python/DLL files to prevent LNK1201
Stop-Process -Name "python", "python3", "Armor3D" -Force -ErrorAction SilentlyContinue

Set-Location $ArmorCore

# 2. Ensure all build tools AND UI dependencies (customtkinter) are installed
python -m pip install --upgrade pip
python -m pip install maturin pyinstaller customtkinter

# 3. Build the Rust extension wheel
python -m maturin build --release

$Wheel = Get-ChildItem -Path $WheelDirectory -Filter "armor_core-*.whl" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if ($null -eq $Wheel) {
    throw "No armor_core wheel was produced in $WheelDirectory"
}

# 4. Force reinstall the newly built wheel into the current Python environment
python -m pip install --force-reinstall --no-deps $Wheel.FullName

# 5. Build the standalone executable with PyInstaller
Set-Location $Root
python -m PyInstaller --clean --noconfirm Armor3D.spec
if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Host "Build complete:" -ForegroundColor Green
Write-Host (Join-Path $Root "dist\Armor3D.exe")
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$ArmorCore = Join-Path $Root "armor-core"
$WheelDirectory = Join-Path $ArmorCore "target\wheels"

Set-Location $ArmorCore

python -m pip install maturin pyinstaller
python -m maturin build --release

$Wheel = Get-ChildItem -Path $WheelDirectory -Filter "armor_core-*.whl" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if ($null -eq $Wheel) {
    throw "No armor_core wheel was produced in $WheelDirectory"
}

python -m pip install --force-reinstall $Wheel.FullName

Set-Location $Root
python -m PyInstaller --clean --noconfirm Armor3D.spec
if ($LASTEXITCODE -ne 0) {
    throw "PyInstaller failed with exit code $LASTEXITCODE"
}

Write-Host ""
Write-Host "Build complete:"
Write-Host (Join-Path $Root "dist\Armor3D.exe")

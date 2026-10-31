# One test round: quit a running cm0102_ui.exe, install the fresh cmui.dll, start the game,
# print cmui.log and save a screenshot. usage: powershell -File run.ps1 [shot.png] [seconds]
# The game folder is $env:CMUI_GAME (default E:\CM0102).
param([string]$Shot = "E:\CM0102\ui\shot.png", [int]$Wait = 10)
$game = if ($env:CMUI_GAME) { $env:CMUI_GAME } else { "E:\CM0102" }
if (Get-Process cm0102_ui -ErrorAction SilentlyContinue) {
    New-Item -Path (Join-Path $game "cmui.quit") -ItemType File -Force | Out-Null
    for ($i = 0; $i -lt 20 -and (Get-Process cm0102_ui -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 500 }
    if (Get-Process cm0102_ui -ErrorAction SilentlyContinue) { Write-Output "game still running: close it by hand"; exit 1 }
}
Remove-Item -LiteralPath (Join-Path $game "cmui.quit"), (Join-Path $game "cmui.log") -ErrorAction SilentlyContinue
Copy-Item "$PSScriptRoot\target\i686-pc-windows-msvc\release\cmui.dll" -Destination (Join-Path $game "cmui.dll") -Force
Start-Process -FilePath (Join-Path $game "cm0102_ui.exe") -WorkingDirectory $game
Start-Sleep $Wait
Get-Content -LiteralPath (Join-Path $game "cmui.log")
python -I "$PSScriptRoot\shot.py" $Shot

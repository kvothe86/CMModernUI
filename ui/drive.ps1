# Drives the running game through cmui's script file and turns its shots into PNGs.
# usage: powershell -File drive.ps1 "536 423" "type VVV" "key 13" "wait 8000" "shot ui\s2.bmp"
# Waits until cmui has logged the last command, then prints the new log lines.
$game = if ($env:CMUI_GAME) { $env:CMUI_GAME } else { "E:\CM0102" }  # the game folder
$before = (Get-Content "$game\cmui.log").Count
# a unique last command to wait for (an earlier line of the log may equal the real last one)
$marker = "mark " + [guid]::NewGuid().ToString("N").Substring(0, 8)
Set-Content "$game\cmui.click" (($args + $marker) -join "`r`n") -Encoding ascii
$last = "script: " + $marker
for ($i = 0; $i -lt 2400 -and -not (@(Get-Content "$game\cmui.log" | Select-Object -Skip $before) -contains $last); $i++) { Start-Sleep -Milliseconds 250 }
Get-Content "$game\cmui.log" | Select-Object -Skip $before
foreach ($a in $args) {
    if ($a -like "shot *") {
        $p = $a.Substring(5)
        $bmp = if ([IO.Path]::IsPathRooted($p)) { $p } else { Join-Path $game $p }
        # the game writes the file a moment after it logs the shot (a slow client: the PNG was
        # converted from nothing and stayed the old one, night 7): wait until it is there and whole
        $size = -1
        for ($w = 0; $w -lt 40; $w++) {
            $now = if (Test-Path $bmp) { (Get-Item $bmp).Length } else { -1 }
            if ($now -gt 0 -and $now -eq $size) { break }
            $size = $now; Start-Sleep -Milliseconds 250
        }
        python -I -c "import sys; from PIL import Image; Image.open(sys.argv[1]).save(sys.argv[1][:-4] + '.png')" $bmp
        Remove-Item $bmp
    }
}

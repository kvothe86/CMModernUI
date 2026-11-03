# cmui_config.ps1: tick cmui's switches and settings, saved in the cmui.ini next to this script.
# Start cmui_config.bat. -Test unticks Tabs, saves to -Ini and writes -Ini + .png (the window).
param([string]$Ini = (Join-Path $PSScriptRoot 'cmui.ini'), [switch]$Test)
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
[Windows.Forms.Application]::EnableVisualStyles()

# the order of FEATURES in src\lib.rs and make_patches.py; a missing name counts as on, as in the game
$switches = @(
    @('theme', 'Theme', 'Dark palette, flat background and flat menu bar.'),
    @('tabs', 'Tabs', 'Flat tab buttons with the active tab marked.'),
    @('attribute_bars', 'Attribute Bars', 'Attributes as bars with the value right of the bar, hidden attributes included.'),
    @('crests', 'Crests', 'Club crests, competition logos and flags.'),
    @('profile', 'Player Profile', 'Info strip, reputation, status and positions panels, CA/PA stars.'),
    @('stats_table', 'Stats Table', 'Player stats table with pills and flat buttons. Honours and Table Icons need it.'),
    @('news', 'News', 'Gold selection, flat dates, story header with crests, clickable club row.'),
    @('club_header', 'Club Header', 'Club header band with crest on club screens.'),
    @('squad_list', 'Squad List', 'Squad list with fitness, form boxes and uniform headers.'),
    @('tactics_pitch', 'Tactics Pitch', 'Tactics pitch with shirts and pills; in a match live condition and position.'),
    @('match_screen', 'Match Screen', 'Crests by the scores, events, key stats, last-5 bar, HALF/FULL TIME.'),
    @('compare_screen', 'Compare Screen', 'P1/P2 boxes, radar, pitches, season. Compare Report needs it.'),
    @('menu_fixes', 'Menu Fixes', 'Menu bar with icons, dark dropdown menus, arrow icons.'),
    @('status_bars', 'Status Bars', 'Morale and condition as bars.'),
    @('search_ability', 'Ability View', 'An Ability view in player search and the squad screen.'),
    @('scout_stars', 'Scout Stars', 'Scouts'' Rec stars from intrinsic weights, rated against the scout''s own club.'),
    @('assistant_pick', 'Assistant Pick', 'An Assistant button on the squad screen: best team, most fit, rotation, clear.'),
    @('team_report', 'Team Report', 'The assistant''s best and backup eleven. Needs Assistant Pick.'),
    @('honours', 'Honours', 'A club''s Honours with cups and counts. Needs Stats Table.'),
    @('export_button', 'Export Button', 'The Print dropdown is called Export and always writes a file.'),
    @('injury_history', 'Injury History', 'A player''s last injuries and bans on Injuries & Bans.'),
    @('transfer_history', 'Transfer History', 'A player''s moves since the game began on the Transfer tab.'),
    @('compare_report', 'Compare Report', 'Lines from the assistant and coaches on Compare Players. Needs Compare Screen.'),
    @('scout_screen', 'Scout Screen', 'A Full scout report chip on scouting stories, with the expected eleven.'),
    @('bigger_text', 'Bigger Text', 'At 1280 wide or more: larger fonts where a text fits its box.'),
    @('squad_table', 'Squad Table', 'At 1280 wide or more: the Position(s) view as a table in two wide columns.'),
    @('table_icons', 'Table Icons', 'League table: a cup and arrows for C, P, Pl and R. Needs Stats Table.'),
    @('background_show', 'Background Show', 'With Background Changes on: the dark panels let the picture through.'),
    @('modern_buttons', 'Modern Buttons', 'Grey buttons, dropdowns and list cells in steel blue, column headers flat.'),
    @('challenge', 'CM Challenge', 'Network game: a button under the menu bar opens the managers'' league.'),
    @('career_goals', 'Career Goals', 'CM Challenge: ten goals for a whole career per manager, picked by the host. Needs CM Challenge.')
)
$extras = @(
    @('hide_ca_pa', 'Hide CA/PA', 'Hide the CA and PA stars on profiles (also the Hide button in the game).'),
    @('profile_details', 'Profile details', 'Start with a profile''s extra panels shown over the attributes; the More / Less button in the game switches the same.')
)

$lines = if (Test-Path -LiteralPath $Ini) { [IO.File]::ReadAllLines($Ini) } else { @() }
$values = @{}
foreach ($l in $lines) { if ($l -match '^\s*([^=]+?)\s*=\s*(.*?)\s*$') { $values[$Matches[1]] = $Matches[2] } }

$dark, $light = [Drawing.Color]::FromArgb(0x1a, 0x1c, 0x20), [Drawing.Color]::FromArgb(0xd8, 0xdc, 0xe2)
$form = New-Object Windows.Forms.Form
$form.Text = "cmui settings - $Ini"
$form.ClientSize = New-Object Drawing.Size(640, 560)
$form.StartPosition = 'CenterScreen'
$form.FormBorderStyle = 'FixedDialog'
$form.MaximizeBox = $false
$form.BackColor = $dark
$form.ForeColor = $light
$form.Font = New-Object Drawing.Font('Segoe UI', 9.5)
$tips = New-Object Windows.Forms.ToolTip
$tips.AutoPopDelay = 20000

# what the switch under the mouse does (the same text as its tooltip)
$help = New-Object Windows.Forms.Label
$help.Location = New-Object Drawing.Point(24, 296)
$help.Size = New-Object Drawing.Size(592, 66)
$help.BorderStyle = 'FixedSingle'
$help.Padding = New-Object Windows.Forms.Padding(6)
$help.Text = 'Point at a switch to see what it does.'
$form.Controls.Add($help)

$boxes = @{}
function Add-Box($item, $x, $y, $on) {
    $b = New-Object Windows.Forms.CheckBox
    $b.Text = $item[1]
    $b.Location = New-Object Drawing.Point($x, $y)
    $b.Size = New-Object Drawing.Size(200, 24)
    $b.Checked = $on
    $tips.SetToolTip($b, $item[2])
    $b.Tag = "$($item[1]): $($item[2])"
    $b.Add_MouseEnter({ $help.Text = $this.Tag })
    $form.Controls.Add($b)
    $boxes[$item[0]] = $b
}
for ($i = 0; $i -lt $switches.Count; $i++) {
    $name = $switches[$i][0]
    # (three columns of eleven: the window is 640 wide)
    Add-Box $switches[$i] (24 + 205 * [math]::Floor($i / 11)) (14 + 25 * ($i % 11)) ($values[$name] -ne '0')
}
for ($i = 0; $i -lt $extras.Count; $i++) {
    Add-Box $extras[$i] (24 + 300 * $i) 374 ($values[$extras[$i][0]] -eq '1')
}

$opacity = 30
if ($values['background_opacity'] -match '^\d+$') { $opacity = [math]::Min([int]$values['background_opacity'], 100) }
$label = New-Object Windows.Forms.Label
$label.Location = New-Object Drawing.Point(24, 412)
$label.Size = New-Object Drawing.Size(230, 24)
$label.Text = "Background opacity: $opacity %"
$form.Controls.Add($label)
$slider = New-Object Windows.Forms.TrackBar
$slider.Location = New-Object Drawing.Point(260, 406)
$slider.Size = New-Object Drawing.Size(350, 40)
$slider.Minimum, $slider.Maximum, $slider.TickFrequency, $slider.Value = 0, 100, 10, $opacity
$slider.Add_ValueChanged({ $label.Text = "Background opacity: $($slider.Value) %" })
$tips.SetToolTip($slider, 'How much of the picture shows through the panels, with Background Changes on and Background Show ticked.')
$form.Controls.Add($slider)

$note = New-Object Windows.Forms.Label
$note.Location = New-Object Drawing.Point(24, 458)
$note.Size = New-Object Drawing.Size(592, 44)
$note.Text = "Switches count from the next start of the game. Opacity counts from the next screen. A switch the game's exe has off stays off."
$form.Controls.Add($note)

function Save-Ini {
    $new = [ordered]@{}
    foreach ($name in $boxes.Keys) { $new[$name] = [string][int]$boxes[$name].Checked }
    $new['background_opacity'] = [string]$slider.Value
    $out = New-Object Collections.ArrayList
    foreach ($l in $lines) {
        if ($l -match '^\s*([^=]+?)\s*=' -and $new.Contains($Matches[1])) {
            [void]$out.Add("$($Matches[1])=$($new[$Matches[1]])")
            $new.Remove($Matches[1])
        } else {
            [void]$out.Add($l) # (what this tool does not know stays)
        }
    }
    foreach ($name in $new.Keys) { [void]$out.Add("$name=$($new[$name])") }
    [IO.File]::WriteAllText($Ini, (($out -join "`r`n") + "`r`n"), (New-Object Text.ASCIIEncoding))
}

$save = New-Object Windows.Forms.Button
$save.Text = 'Save'
$save.Location = New-Object Drawing.Point(420, 514)
$save.Size = New-Object Drawing.Size(92, 32)
$save.BackColor = [Drawing.Color]::FromArgb(0x8a, 0x70, 0x34)
$save.Add_Click({ Save-Ini; $form.Close() })
$form.Controls.Add($save)
$cancel = New-Object Windows.Forms.Button
$cancel.Text = 'Cancel'
$cancel.Location = New-Object Drawing.Point(524, 514)
$cancel.Size = New-Object Drawing.Size(92, 32)
$cancel.Add_Click({ $form.Close() })
$form.Controls.Add($cancel)
$form.AcceptButton, $form.CancelButton = $save, $cancel

if ($Test) {
    $boxes['tabs'].Checked = $false
    $help.Text = $boxes['career_goals'].Tag
    $slider.Value = 45
    Save-Ini
    $form.StartPosition = 'Manual'
    $form.Location = New-Object Drawing.Point(-3000, 0)
    $form.Show()
    [Windows.Forms.Application]::DoEvents()
    $picture = New-Object Drawing.Bitmap($form.Width, $form.Height)
    $form.DrawToBitmap($picture, (New-Object Drawing.Rectangle(0, 0, $form.Width, $form.Height)))
    $picture.Save("$Ini.png")
    $form.Close()
    return
}
[void]$form.ShowDialog()

# Options window for convert3d. Reads and writes the JSON paths passed on the
# command line. Exit 0 after Save, exit 2 if the window is closed without saving.
param(
    [Parameter(Mandatory = $true)][string]$InputPath,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = "Stop"
trap {
    [Console]::Error.WriteLine($_)
    exit 1
}

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()

$data = Get-Content -LiteralPath $InputPath -Raw -Encoding UTF8 | ConvertFrom-Json

$form = New-Object System.Windows.Forms.Form
$form.Text = "Convert 3D Options"
$form.ClientSize = New-Object System.Drawing.Size(980, 816)
$form.MinimumSize = New-Object System.Drawing.Size(820, 636)
$form.StartPosition = "CenterScreen"
$form.Font = New-Object System.Drawing.Font("Segoe UI", 9)

$check = New-Object System.Windows.Forms.CheckBox
$check.Text = "Let user specify the output folder before any conversion"
$check.AutoSize = $true
$check.Location = New-Object System.Drawing.Point(12, 10)
$check.Checked = [bool]$data.ask_output
$form.Controls.Add($check)

$hint = New-Object System.Windows.Forms.Label
$hint.Location = New-Object System.Drawing.Point(12, 34)
$hint.Size = New-Object System.Drawing.Size(956, 78)
$hint.Text = @"
When this is unchecked, the converted file is saved in the same folder as the original.

One step is one application reading a format and writing another. With Maya, .ma to .usda is Maya (writes .fbx) then Blender (writes .usda). Aztek reads and writes .ma, .mb, .obj, .fbx and USD itself, so that conversion is one step. Paths below are shared. A path on one file type replaces only that read or write. Any other program is run as: program.exe input output. Clear a field to use the shared application again.
"@
$form.Controls.Add($hint)

$exist = New-Object System.Windows.Forms.GroupBox
$exist.Text = "When the output already exists"
$exist.Location = New-Object System.Drawing.Point(12, 116)
$exist.Size = New-Object System.Drawing.Size(956, 108)
$form.Controls.Add($exist)

$mode = if ($data.existing) { [string]$data.existing } else { "ask" }
function New-Mode([string]$text, [string]$value, [int]$y) {
    $radio = New-Object System.Windows.Forms.RadioButton
    $radio.Text = $text
    $radio.AutoSize = $true
    $radio.Location = New-Object System.Drawing.Point(12, $y)
    $radio.Checked = ($mode -eq $value)
    $exist.Controls.Add($radio) | Out-Null
    return $radio
}
$rAsk = New-Mode "Ask before conversion" "ask" 18
$rOver = New-Mode "Always overwrite" "overwrite" 40
$rInc = New-Mode "Increment the name (file_(2))" "increment" 62
$existNote = New-Object System.Windows.Forms.Label
$existNote.Text = "With a custom folder, both questions are the same window."
$existNote.AutoSize = $true
$existNote.Location = New-Object System.Drawing.Point(280, 22)
$exist.Controls.Add($existNote)

$apps = New-Object System.Windows.Forms.GroupBox
$apps.Text = "Applications"
$apps.Location = New-Object System.Drawing.Point(12, 232)
$toolCount = @($data.tools).Count
$apps.Size = New-Object System.Drawing.Size(956, (30 + $toolCount * 28))
$form.Controls.Add($apps)

$script:formatBoxes = New-Object System.Collections.Generic.List[System.Windows.Forms.TextBox]
$script:reads = @{}
$script:writes = @{}
$script:toolBoxes = @{}

function Update-Shared([System.Windows.Forms.TextBox]$toolBox) {
    $prev = [string]$toolBox.Tag
    $now = $toolBox.Text
    if ($prev -and ($prev -ne $now)) {
        foreach ($tb in $script:formatBoxes) {
            if ($tb.Text -eq $prev) { $tb.Text = $now }
        }
    }
    $toolBox.Tag = $now
}

function Browse-Into([System.Windows.Forms.TextBox]$box, [bool]$shared) {
    $dialog = New-Object System.Windows.Forms.OpenFileDialog
    $dialog.CheckFileExists = $true
    $dialog.Filter = "Programs (*.exe)|*.exe|All files (*.*)|*.*"
    if ($box.Text -and (Test-Path -LiteralPath $box.Text)) {
        $dialog.FileName = $box.Text
    }
    if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
        $box.Text = $dialog.FileName
        if ($shared) { Update-Shared $box }
    }
}

$y = 22
foreach ($tool in @($data.tools)) {
    $label = New-Object System.Windows.Forms.Label
    $label.Text = [string]$tool.label
    $label.Location = New-Object System.Drawing.Point(12, ($y + 4))
    $label.Size = New-Object System.Drawing.Size(150, 20)
    $apps.Controls.Add($label)

    $box = New-Object System.Windows.Forms.TextBox
    $box.Text = [string]$tool.path
    $box.Tag = [string]$tool.path
    $box.Location = New-Object System.Drawing.Point(168, $y)
    $box.Size = New-Object System.Drawing.Size(720, 22)
    $box.Add_Leave({ Update-Shared $this })
    $apps.Controls.Add($box)
    $script:toolBoxes[[string]$tool.id] = $box

    $browse = New-Object System.Windows.Forms.Button
    $browse.Text = "..."
    $browse.Tag = $box
    $browse.Location = New-Object System.Drawing.Point(896, ($y - 1))
    $browse.Size = New-Object System.Drawing.Size(32, 24)
    $browse.Add_Click({ Browse-Into $this.Tag $true })
    $apps.Controls.Add($browse)
    $y += 28
}

$fileTop = $apps.Location.Y + $apps.Size.Height + 8
$per = New-Object System.Windows.Forms.Label
$per.Text = "File types"
$per.Location = New-Object System.Drawing.Point(12, $fileTop)
$per.AutoSize = $true
$form.Controls.Add($per)

function Add-Header([string]$text, [int]$x, [int]$y) {
    $header = New-Object System.Windows.Forms.Label
    $header.Text = $text
    $header.Location = New-Object System.Drawing.Point($x, $y)
    $header.AutoSize = $true
    $form.Controls.Add($header)
}
Add-Header "Read" 280 ($fileTop + 20)
Add-Header "Write" 620 ($fileTop + 20)

$panelTop = $fileTop + 40
$panel = New-Object System.Windows.Forms.Panel
$panel.Location = New-Object System.Drawing.Point(12, $panelTop)
$panel.Size = New-Object System.Drawing.Size(956, (768 - $panelTop))
$panel.AutoScroll = $true
$panel.Anchor = [System.Windows.Forms.AnchorStyles]::Top -bor [System.Windows.Forms.AnchorStyles]::Bottom -bor [System.Windows.Forms.AnchorStyles]::Left -bor [System.Windows.Forms.AnchorStyles]::Right
$form.Controls.Add($panel)

$rowY = 4
foreach ($row in @($data.rows)) {
    $ext = [string]$row.ext
    $name = New-Object System.Windows.Forms.Label
    $name.Text = "$($row.name) (.$ext)"
    $name.Location = New-Object System.Drawing.Point(4, ($rowY + 4))
    $name.Size = New-Object System.Drawing.Size(250, 20)
    $name.AutoEllipsis = $true
    $panel.Controls.Add($name)

    foreach ($side in @("read", "write")) {
        $enabled = if ($side -eq "read") { [bool]$row.can_read } else { [bool]$row.can_write }
        $x = if ($side -eq "read") { 260 } else { 600 }
        $box = New-Object System.Windows.Forms.TextBox
        $box.Text = if ($side -eq "read") { [string]$row.read } else { [string]$row.write }
        $box.Location = New-Object System.Drawing.Point($x, $rowY)
        $box.Size = New-Object System.Drawing.Size(280, 22)
        $box.Enabled = $enabled
        $panel.Controls.Add($box)
        if ($enabled) { [void]$script:formatBoxes.Add($box) }
        if ($side -eq "read") { $script:reads[$ext] = $box } else { $script:writes[$ext] = $box }

        $browse = New-Object System.Windows.Forms.Button
        $browse.Text = "..."
        $browse.Tag = $box
        $browse.Location = New-Object System.Drawing.Point(($x + 284), ($rowY - 1))
        $browse.Size = New-Object System.Drawing.Size(32, 24)
        $browse.Enabled = $enabled
        $browse.Add_Click({ Browse-Into $this.Tag $false })
        $panel.Controls.Add($browse)
    }
    $rowY += 28
}
$panel.AutoScrollMinSize = New-Object System.Drawing.Size(900, ($rowY + 8))

$cancel = New-Object System.Windows.Forms.Button
$cancel.Text = "Cancel"
$cancel.Size = New-Object System.Drawing.Size(88, 28)
$cancel.Location = New-Object System.Drawing.Point(788, 776)
$cancel.Anchor = [System.Windows.Forms.AnchorStyles]::Bottom -bor [System.Windows.Forms.AnchorStyles]::Right
$cancel.Add_Click({ $form.Close() })
$form.Controls.Add($cancel)
$form.CancelButton = $cancel

$save = New-Object System.Windows.Forms.Button
$save.Text = "Save"
$save.Size = New-Object System.Drawing.Size(88, 28)
$save.Location = New-Object System.Drawing.Point(880, 776)
$save.Anchor = [System.Windows.Forms.AnchorStyles]::Bottom -bor [System.Windows.Forms.AnchorStyles]::Right
$form.Controls.Add($save)
$form.AcceptButton = $save

$script:saved = $false
$save.Add_Click({
    $toolOut = @()
    foreach ($tool in @($data.tools)) {
        $id = [string]$tool.id
        $toolOut += [pscustomobject]@{ id = $id; path = $script:toolBoxes[$id].Text.Trim() }
    }
    $rowOut = @()
    foreach ($row in @($data.rows)) {
        $ext = [string]$row.ext
        $rowOut += [pscustomobject]@{
            ext   = $ext
            read  = $script:reads[$ext].Text.Trim()
            write = $script:writes[$ext].Text.Trim()
        }
    }
    $toolsJson = ($toolOut | ConvertTo-Json -Depth 5 -Compress)
    if (@($toolOut).Count -eq 1) { $toolsJson = "[$toolsJson]" }
    $rowsJson = ($rowOut | ConvertTo-Json -Depth 5 -Compress)
    if (@($rowOut).Count -eq 1) { $rowsJson = "[$rowsJson]" }
    $ask = if ($check.Checked) { "true" } else { "false" }
    $existing = if ($rAsk.Checked) { "ask" } elseif ($rOver.Checked) { "overwrite" } else { "increment" }
    $json = "{`"ask_output`":$ask,`"existing`":`"$existing`",`"tools`":$toolsJson,`"rows`":$rowsJson}"
    [System.IO.File]::WriteAllText($OutputPath, $json, (New-Object System.Text.UTF8Encoding $false))
    $script:saved = $true
    $form.Close()
})

[void]$form.ShowDialog()
if (-not $script:saved) { exit 2 }
exit 0

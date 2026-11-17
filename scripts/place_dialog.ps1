# One confirmation before a conversion. It can ask for a folder, ask whether to
# overwrite a name that already exists, or both in the same window.
# Exit 0 writes {"folder","overwrite"}. Exit 2 cancels.
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
$askFolder = [bool]$data.ask_folder
$askConflict = [bool]$data.ask_conflict

function Existing-Names([string]$folder) {
    $found = @()
    if (-not $folder -or -not (Test-Path -LiteralPath $folder -PathType Container)) {
        return @()
    }
    foreach ($job in @($data.jobs)) {
        if (-not $job) { continue }
        foreach ($name in @($job.names)) {
            if (-not $name) { continue }
            if (Test-Path -LiteralPath (Join-Path $folder ([string]$name))) {
                $found += [string]$name
            }
        }
    }
    return @($found | Select-Object -Unique)
}

$form = New-Object System.Windows.Forms.Form
$form.Text = "Convert 3D"
$form.FormBorderStyle = "FixedDialog"
$form.StartPosition = "CenterScreen"
$form.MaximizeBox = $false
$form.MinimizeBox = $false
$form.Font = New-Object System.Drawing.Font("Segoe UI", 9)

$y = 12
$folderBox = $null
if ($askFolder) {
    $folderLabel = New-Object System.Windows.Forms.Label
    $folderLabel.Text = "Save converted files to"
    $folderLabel.Location = New-Object System.Drawing.Point(12, $y)
    $folderLabel.AutoSize = $true
    $form.Controls.Add($folderLabel)
    $y += 22
    $folderBox = New-Object System.Windows.Forms.TextBox
    $folderBox.Text = [string]$data.folder
    $folderBox.Location = New-Object System.Drawing.Point(12, $y)
    $folderBox.Size = New-Object System.Drawing.Size(360, 22)
    $form.Controls.Add($folderBox)
    $browse = New-Object System.Windows.Forms.Button
    $browse.Text = "Browse..."
    $browse.Location = New-Object System.Drawing.Point(380, ($y - 1))
    $browse.Size = New-Object System.Drawing.Size(84, 24)
    $form.Controls.Add($browse)
    $y += 36
}

$warn = New-Object System.Windows.Forms.Panel
$warn.Location = New-Object System.Drawing.Point(12, $y)
$warn.Size = New-Object System.Drawing.Size(452, $(if ($askFolder) { 108 } else { 64 }))
$form.Controls.Add($warn)

$icon = New-Object System.Windows.Forms.PictureBox
$icon.Image = [System.Drawing.SystemIcons]::Warning.ToBitmap()
$icon.Location = New-Object System.Drawing.Point(0, 4)
$icon.Size = New-Object System.Drawing.Size(32, 32)
$warn.Controls.Add($icon)

$msg = New-Object System.Windows.Forms.Label
$msg.Location = New-Object System.Drawing.Point(44, 0)
$msg.Size = New-Object System.Drawing.Size(400, 52)
$warn.Controls.Add($msg)

$overwriteRadio = New-Object System.Windows.Forms.RadioButton
$overwriteRadio.Text = "Overwrite"
$overwriteRadio.Location = New-Object System.Drawing.Point(44, 58)
$overwriteRadio.AutoSize = $true
$overwriteRadio.Visible = $askFolder
$warn.Controls.Add($overwriteRadio)

$incrementRadio = New-Object System.Windows.Forms.RadioButton
$incrementRadio.Text = "Increment (file_(2))"
$incrementRadio.Location = New-Object System.Drawing.Point(160, 58)
$incrementRadio.AutoSize = $true
$incrementRadio.Checked = $true
$incrementRadio.Visible = $askFolder
$warn.Controls.Add($incrementRadio)

function Update-Warning {
    if (-not $askConflict) {
        $warn.Visible = $false
        return
    }
    $folder = if ($folderBox) { $folderBox.Text.Trim() } else { [string]$data.folder }
    $found = @(Existing-Names $folder)
    if ($found.Count -eq 0) {
        $warn.Visible = $false
        return
    }
    $shown = @($found | Select-Object -First 3) -join ", "
    if ($found.Count -gt 3) { $shown = "$shown and $($found.Count - 3) more" }
    $msg.Text = "$shown already exists.`r`nOverwrite replaces it. Increment keeps it and adds `"_(2)`" to the name."
    $warn.Visible = $true
}

if ($folderBox) { $folderBox.Add_TextChanged({ Update-Warning }) }
Update-Warning

function Finish([bool]$overwrite) {
    $folder = if ($folderBox) { $folderBox.Text.Trim() } else { [string]$data.folder }
    if ($askFolder -and -not $folder) { return }
    $flag = if ($overwrite) { "true" } else { "false" }
    $json = "{`"folder`":$(ConvertTo-Json $folder),`"overwrite`":$flag}"
    [System.IO.File]::WriteAllText($OutputPath, $json, (New-Object System.Text.UTF8Encoding $false))
    $form.DialogResult = [System.Windows.Forms.DialogResult]::OK
    $form.Close()
}

$buttonY = if ($askFolder -and $askConflict) { $warn.Location.Y + $warn.Height + 12 } elseif ($warn.Visible) { $warn.Location.Y + $warn.Height + 12 } else { $y }
if ($askFolder) {
    $convert = New-Object System.Windows.Forms.Button
    $convert.Text = "Convert"
    $convert.Location = New-Object System.Drawing.Point(292, $buttonY)
    $convert.Size = New-Object System.Drawing.Size(88, 28)
    $convert.Add_Click({ Finish ($warn.Visible -and $overwriteRadio.Checked) })
    $form.Controls.Add($convert)
    $form.AcceptButton = $convert
    $cancel = New-Object System.Windows.Forms.Button
    $cancel.Text = "Cancel"
    $cancel.Location = New-Object System.Drawing.Point(388, $buttonY)
    $cancel.Size = New-Object System.Drawing.Size(76, 28)
    $cancel.Add_Click({ $form.Close() })
    $form.Controls.Add($cancel)
    $form.CancelButton = $cancel
} else {
    $overwriteBtn = New-Object System.Windows.Forms.Button
    $overwriteBtn.Text = "Overwrite"
    $overwriteBtn.Location = New-Object System.Drawing.Point(196, $buttonY)
    $overwriteBtn.Size = New-Object System.Drawing.Size(88, 28)
    $overwriteBtn.Add_Click({ Finish $true })
    $form.Controls.Add($overwriteBtn)
    $incrementBtn = New-Object System.Windows.Forms.Button
    $incrementBtn.Text = "Increment"
    $incrementBtn.Location = New-Object System.Drawing.Point(292, $buttonY)
    $incrementBtn.Size = New-Object System.Drawing.Size(88, 28)
    $incrementBtn.Add_Click({ Finish $false })
    $form.Controls.Add($incrementBtn)
    $form.AcceptButton = $incrementBtn
    $cancel = New-Object System.Windows.Forms.Button
    $cancel.Text = "Cancel"
    $cancel.Location = New-Object System.Drawing.Point(388, $buttonY)
    $cancel.Size = New-Object System.Drawing.Size(76, 28)
    $cancel.Add_Click({ $form.Close() })
    $form.Controls.Add($cancel)
    $form.CancelButton = $cancel
}

if ($folderBox) {
    $browse.Add_Click({
        $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
        $dialog.Description = "Save converted files to"
        $dialog.ShowNewFolderButton = $true
        if ($folderBox.Text -and (Test-Path -LiteralPath $folderBox.Text -PathType Container)) {
            $dialog.SelectedPath = $folderBox.Text
        }
        if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
            $folderBox.Text = $dialog.SelectedPath
        }
    })
}

$form.ClientSize = New-Object System.Drawing.Size(476, ($buttonY + 44))
[void]$form.ShowDialog()
if (-not (Test-Path -LiteralPath $OutputPath)) { exit 2 }
exit 0

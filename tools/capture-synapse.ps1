# Guided capture of what Razer Synapse sends to a headset, for rzr.
#
# Run it in PowerShell, from this file's folder, with Synapse installed and the
# headset connected:
#
#   powershell -ExecutionPolicy Bypass -File .\capture-synapse.ps1
#
# It asks you to change one setting at a time in Synapse and to press Enter
# after each. It notes the time of every step, takes a snapshot of the Razer
# audio devices' registry keys (where Windows audio effects such as THX keep
# their settings), and at the end copies Synapse's logs, which list every
# command Synapse sent. Everything ends up in "rzr-capture" and
# "rzr-capture.zip" on your Desktop.
#
# It only READS: it never changes the registry, a setting or a file outside
# its own folder. Synapse's logs can contain your Windows user name and the
# headset's serial number: check the zip before sharing it publicly.
# Decode it with: python3 decode-synapse-log.py rzr-capture

$ErrorActionPreference = 'Continue'
$out = Join-Path ([Environment]::GetFolderPath('Desktop')) 'rzr-capture'
New-Item -ItemType Directory -Force $out | Out-Null
$steps = Join-Path $out 'steps.txt'
$start = Get-Date

# Registry keys of the Razer audio endpoints (outputs and microphones).
function Get-RazerEndpoints {
    foreach ($flow in 'Render', 'Capture') {
        $base = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$flow"
        Get-ChildItem $base -ErrorAction SilentlyContinue | ForEach-Object {
            $props = Get-ItemProperty (Join-Path $_.PSPath 'Properties') -ErrorAction SilentlyContinue
            $text = ($props.PSObject.Properties | ForEach-Object { "$($_.Value)" }) -join ' '
            if ($text -match 'Razer|BlackShark|Kraken|Barracuda|Nari|Blade') {
                "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$flow\$($_.PSChildName)"
            }
        }
    }
}

function Save-Snapshot([string]$name) {
    $file = Join-Path $out "registry-$name.txt"
    "# $name  $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')" | Out-File $file -Encoding utf8
    $keys = @(Get-RazerEndpoints)
    if ($keys.Count -eq 0) {
        '!! No Razer audio device found. Is the headset connected?' | Out-File $file -Append -Encoding utf8
    }
    foreach ($k in $keys) {
        reg query $k /s 2>&1 | Out-File $file -Append -Encoding utf8
    }
    reg query 'HKCU\Software\THX' /s 2>$null | Out-File $file -Append -Encoding utf8
}

function Log([string]$line) {
    "$(Get-Date -Format 'HH:mm:ss.fff')  $line" | Out-File $steps -Append -Encoding utf8
}

$list = @(
    'Open Synapse on your headset''s page, with the headset ON and connected. Close rzr if it is open. Do not change anything yet.',
    'EQ: pick a different preset.',
    'EQ: pick another preset.',
    'EQ: pick the Custom (editable) preset.',
    'EQ: drag the 1 kHz band (or the middle one) all the way up.',
    'EQ: drag that band back to 0.',
    'EQ: if there is a second group of presets (e.g. Esports), pick one of them.',
    'MIC MONITORING / SIDETONE: turn it on.',
    'MIC MONITORING: move its slider to about 50.',
    'MIC MONITORING: turn it off.',
    'MIC: change the microphone volume.',
    'MIC: turn on one enhancement (noise reduction, normalization...), then turn it off again.',
    'POWER: turn on auto power-off / power saving and move its slider.',
    'LIGHTING (if any): change the colour or the effect.',
    'OTHER: change any other setting your headset has. Type its name before pressing Enter.',
    'HEADSET: press its EQ or mode button once (if it has one).',
    'HEADSET: mute the microphone with its button, then unmute it.',
    'HEADSET: turn it off, wait 5 seconds, turn it on again.'
)

Write-Host ''
Write-Host 'Guided Synapse capture for rzr' -ForegroundColor Green
Write-Host 'Make each change in Synapse, wait 2 seconds and press Enter.'
Write-Host 'Type a note before Enter if something is worth saying (for example what you heard).'
Write-Host 'Type s and Enter to skip an option your headset or Synapse does not have.'
Write-Host ''
Save-Snapshot '00-start'
Log '00  start'

for ($i = 0; $i -lt $list.Count; $i++) {
    $n = '{0:D2}' -f ($i + 1)
    Write-Host "[$n/$($list.Count)] $($list[$i])" -ForegroundColor Cyan
    $answer = Read-Host '   Enter when done (s = skip)'
    if ($answer -eq 's') {
        Log "$n  SKIPPED  $($list[$i])"
        continue
    }
    Log "$n  $($list[$i])"
    if ($answer) { Log "$n  NOTE: $answer" }
    Save-Snapshot "step-$n"
}

Write-Host ''
Write-Host 'Copying Synapse logs...' -ForegroundColor Green
# Only files written since the capture started (smaller zip).
$since = $start.AddMinutes(-10)
$sources = @{ 'local' = "$env:LOCALAPPDATA\Razer"; 'programdata' = "$env:ProgramData\Razer" }
foreach ($name in $sources.Keys) {
    $root = $sources[$name]
    if (-not (Test-Path $root)) { continue }
    Get-ChildItem $root -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -ge $since -and $_.Extension -match '^\.(log|txt)$' } |
        ForEach-Object {
            $rel = $_.FullName.Substring($root.Length).TrimStart('\')
            $dest = Join-Path $out "logs\$name\$rel"
            New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
            Copy-Item $_.FullName $dest -ErrorAction SilentlyContinue
        }
}

$zip = "$out.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "$out\*" -DestinationPath $zip
Write-Host ''
Write-Host "Done: $zip" -ForegroundColor Green
Write-Host 'Check it for personal data, then attach it to your headset report on GitHub.'

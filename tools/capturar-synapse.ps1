# Captura de datos de Razer Synapse para rzr.
#
# Uso (en PowerShell, desde la carpeta de este archivo):
#   1) ANTES de instalar Synapse:   powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase antes
#   2) Con Synapse instalado:       powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase synapse
#
# Todo queda en el Escritorio, en la carpeta "rzr-captura" y en "rzr-captura.zip".
# El script solo LEE el registro y copia los logs de Synapse: no modifica nada.

param(
    [ValidateSet('antes', 'synapse')]
    [string]$Fase = 'synapse'
)

$ErrorActionPreference = 'Continue'
$out = Join-Path ([Environment]::GetFolderPath('Desktop')) 'rzr-captura'
New-Item -ItemType Directory -Force $out | Out-Null
$timeline = Join-Path $out 'pasos.txt'
$inicio = Get-Date

function Get-HeadsetEndpoints {
    foreach ($flow in 'Render', 'Capture') {
        $base = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$flow"
        Get-ChildItem $base -ErrorAction SilentlyContinue | ForEach-Object {
            $props = Get-ItemProperty (Join-Path $_.PSPath 'Properties') -ErrorAction SilentlyContinue
            $text = ($props.PSObject.Properties | ForEach-Object { "$($_.Value)" }) -join ' '
            if ($text -match 'BlackShark|Razer') {
                "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\$flow\$($_.PSChildName)"
            }
        }
    }
}

# Foto del registro de audio del headset (incluye los efectos/APO de Windows).
function Save-Snapshot([string]$name) {
    $file = Join-Path $out "$name.txt"
    "# $name  $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff')" | Out-File $file -Encoding utf8
    $keys = @(Get-HeadsetEndpoints)
    if ($keys.Count -eq 0) {
        "!! No se encontraron dispositivos de audio del headset. Esta conectado el dongle?" | Out-File $file -Append -Encoding utf8
    }
    foreach ($k in $keys) {
        reg query $k /s 2>&1 | Out-File $file -Append -Encoding utf8
    }
}

function Log([string]$line) {
    "$(Get-Date -Format 'HH:mm:ss.fff')  $line" | Out-File $timeline -Append -Encoding utf8
}

if ($Fase -eq 'antes') {
    Save-Snapshot '00-antes-de-synapse'
    reg query 'HKLM\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects' /s 2>&1 |
        Out-File (Join-Path $out 'apos-antes.txt') -Encoding utf8
    Log '00  Foto antes de instalar Synapse'
    Write-Host ''
    Write-Host 'Listo. Ahora instala Razer Synapse, conecta el headset y ejecuta:' -ForegroundColor Green
    Write-Host '  powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase synapse'
    exit
}

$pasos = @(
    'Abre Synapse en AUDIO con el headset ENCENDIDO y conectado por el dongle. No cambies nada todavia.',
    'MICROFONO: activa MIC MONITORING (sidetone).',
    'MICROFONO: mueve el slider de MIC MONITORING a 50.',
    'MICROFONO: desactiva MIC MONITORING.',
    'MEJORA / ENHANCEMENT: activa BASS BOOST.',
    'MEJORA: mueve BASS BOOST al maximo.',
    'MEJORA: mueve BASS BOOST al minimo.',
    'MEJORA: desactiva BASS BOOST.',
    'MEJORA: activa SOUND NORMALIZATION.',
    'MEJORA: mueve SOUND NORMALIZATION al maximo.',
    'MEJORA: desactiva SOUND NORMALIZATION.',
    'MEJORA: activa VOICE CLARITY.',
    'MEJORA: mueve VOICE CLARITY al maximo.',
    'MEJORA: desactiva VOICE CLARITY.',
    'SONIDO: activa THX SPATIAL AUDIO.',
    'SONIDO: vuelve a ESTEREO.',
    'SONIDO: selecciona el preset PELICULA / MOVIE.',
    'SONIDO: con PELICULA seleccionado, arrastra el punto de 1kHz hasta arriba.',
    'SONIDO: selecciona PERSONALIZADO / CUSTOM.',
    'MICROFONO: activa VOLUME NORMALIZATION.',
    'MICROFONO: desactiva VOLUME NORMALIZATION.',
    'MICROFONO: activa VOCAL CLARITY y mueve su slider al maximo.',
    'MICROFONO: desactiva VOCAL CLARITY.',
    'MICROFONO: activa AMBIENT NOISE REDUCTION y mueve su slider al maximo.',
    'MICROFONO: desactiva AMBIENT NOISE REDUCTION.',
    'MICROFONO: activa MIC SENSITIVITY / VOICE GATE y mueve su slider al maximo.',
    'MICROFONO: desactiva MIC SENSITIVITY / VOICE GATE.',
    'MICROFONO: en MIC EQUALIZER sube la banda de 1kHz al maximo.',
    'MICROFONO: pulsa MIC PREVIEW, habla 3 segundos y vuelve a pulsarlo.',
    'MICROFONO: mueve MIC VOLUME a 50.',
    'POWER / ALIMENT.: activa el ahorro de energia / apagado automatico y mueve su slider.',
    'MEJORA: activa DO NOT DISTURB y luego desactivalo.',
    'Pulsa UNA vez el boton fisico de EQ del headset.'
)

Write-Host ''
Write-Host 'Captura guiada de Synapse para rzr' -ForegroundColor Green
Write-Host 'Haz cada cambio en Synapse, espera 2 segundos y presiona Enter.'
Write-Host 'Si una opcion no existe en tu Synapse, escribe s y Enter para saltarla.'
Write-Host 'No tengas rzr abierto mientras capturas.'
Write-Host ''

Save-Snapshot '00b-synapse-instalado'
Log '00b Synapse instalado, antes de los pasos'

for ($i = 0; $i -lt $pasos.Count; $i++) {
    $n = '{0:D2}' -f ($i + 1)
    Write-Host "[$n/$($pasos.Count)] $($pasos[$i])" -ForegroundColor Cyan
    $r = Read-Host '   Enter cuando lo hayas hecho (s = saltar)'
    if ($r -eq 's') {
        Log "$n  SALTADO  $($pasos[$i])"
        continue
    }
    Log "$n  $($pasos[$i])"
    Save-Snapshot "paso-$n"
}

Write-Host ''
Write-Host 'Copiando logs de Synapse...' -ForegroundColor Green
reg query 'HKLM\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects' /s 2>&1 |
    Out-File (Join-Path $out 'apos-con-synapse.txt') -Encoding utf8

# Solo los archivos escritos desde que empezo la captura (menos tamano).
$desde = $inicio.AddMinutes(-10)
$fuentes = @{ 'local' = "$env:LOCALAPPDATA\Razer"; 'programdata' = "$env:ProgramData\Razer" }
foreach ($nombre in $fuentes.Keys) {
    $root = $fuentes[$nombre]
    if (-not (Test-Path $root)) { continue }
    Get-ChildItem $root -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -ge $desde -and $_.Extension -match '^\.(log|txt)$' } |
        ForEach-Object {
            $rel = $_.FullName.Substring($root.Length).TrimStart('\')
            $dest = Join-Path $out "logs\$nombre\$rel"
            New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
            Copy-Item $_.FullName $dest -ErrorAction SilentlyContinue
        }
}

$zip = "$out.zip"
if (Test-Path $zip) { Remove-Item $zip }
Compress-Archive -Path "$out\*" -DestinationPath $zip
Write-Host ''
Write-Host "Listo: $zip" -ForegroundColor Green
Write-Host 'Enviame ese archivo. Ya puedes desinstalar Synapse.'

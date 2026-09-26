# Captura de datos de Razer Synapse para rzr.
#
# Uso (en PowerShell, desde la carpeta de este archivo):
#   1) ANTES de instalar Synapse:   powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase antes
#   2) Con Synapse instalado:       powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase synapse
#   3) Synapse DESINSTALADO y PC reiniciado (mejoras de audio de Windows):
#                                   powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase windows
#
# Todo queda en el Escritorio, en la carpeta "rzr-captura" y en "rzr-captura.zip"
# ("rzr-captura-windows" y su .zip en la fase windows).
# El script solo LEE el registro y copia los logs de Synapse: no modifica nada.

param(
    [ValidateSet('antes', 'synapse', 'windows')]
    [string]$Fase = 'synapse'
)

$ErrorActionPreference = 'Continue'
$carpeta = if ($Fase -eq 'windows') { 'rzr-captura-windows' } else { 'rzr-captura' }
$out = Join-Path ([Environment]::GetFolderPath('Desktop')) $carpeta
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

if ($Fase -eq 'windows') {
    $pasos = @(
        'Abre el Panel de sonido: tecla Windows + R, escribe mmsys.cpl y Enter. En Reproduccion, doble clic en los auriculares BlackShark. Ve a la pestana Mejoras / Enhancements. (Si no existe esa pestana, escribe s en todos los pasos y avisame.) No cambies nada todavia.',
        'Mejoras: activa BASS BOOST / REFUERZO DE GRAVES y pulsa Aplicar.',
        'Mejoras: con Bass Boost seleccionado pulsa Configuracion, cambia la frecuencia y el nivel de refuerzo, Aceptar y Aplicar.',
        'Mejoras: desactiva BASS BOOST y pulsa Aplicar.',
        'Mejoras: activa LOUDNESS EQUALIZATION / IGUALACION DE SONORIDAD y pulsa Aplicar.',
        'Mejoras: con Loudness Equalization seleccionado pulsa Configuracion, cambia el tiempo de liberacion, Aceptar y Aplicar.',
        'Mejoras: desactiva LOUDNESS EQUALIZATION y pulsa Aplicar.',
        'Mejoras: activa VIRTUAL SURROUND / SONIDO ENVOLVENTE VIRTUAL y pulsa Aplicar.',
        'Mejoras: desactiva VIRTUAL SURROUND y pulsa Aplicar.',
        'Mejoras: activa ROOM CORRECTION / CORRECCION DE SALA si existe (si abre un asistente, cancelalo) y pulsa Aplicar.',
        'Mejoras: desactiva ROOM CORRECTION y pulsa Aplicar.',
        'Mejoras: marca DESHABILITAR TODAS LAS MEJORAS y pulsa Aplicar.',
        'Mejoras: desmarca DESHABILITAR TODAS LAS MEJORAS y pulsa Aplicar. Cierra esa ventana.',
        'Configuracion de Windows > Sistema > Sonido > tu microfono BlackShark: si ves Mejoras de audio o Claridad de voz, activalo.',
        'Vuelve a dejar esa opcion del microfono como estaba.'
    )
} else {
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
}

Write-Host ''
if ($Fase -eq 'windows') {
    Write-Host 'Captura guiada de las mejoras de audio de Windows para rzr' -ForegroundColor Green
    Write-Host 'Haz cada cambio, espera 2 segundos y presiona Enter.'
    Write-Host 'Si una opcion no existe, escribe s y Enter para saltarla.'
    Write-Host 'Pon algo de musica: asi Windows aplica cada cambio de verdad.'
    Write-Host ''
    Save-Snapshot '00-windows-sin-synapse'
    Log '00  Windows sin Synapse, antes de los pasos'
} else {
Write-Host 'Captura guiada de Synapse para rzr' -ForegroundColor Green
Write-Host 'Haz cada cambio en Synapse, espera 2 segundos y presiona Enter.'
Write-Host 'Si una opcion no existe en tu Synapse, escribe s y Enter para saltarla.'
Write-Host 'No tengas rzr abierto mientras capturas.'
Write-Host ''

Save-Snapshot '00b-synapse-instalado'
Log '00b Synapse instalado, antes de los pasos'
}

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

if ($Fase -eq 'windows') {
    reg query 'HKLM\SOFTWARE\Classes\AudioEngine\AudioProcessingObjects' /s 2>&1 |
        Out-File (Join-Path $out 'apos-windows.txt') -Encoding utf8
    $zip = "$out.zip"
    if (Test-Path $zip) { Remove-Item $zip }
    Compress-Archive -Path "$out\*" -DestinationPath $zip
    Write-Host ''
    Write-Host "Listo: $zip" -ForegroundColor Green
    Write-Host 'Enviame ese archivo.'
    exit
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

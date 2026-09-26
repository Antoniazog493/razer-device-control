# Captura de datos de Razer Synapse para rzr.
#
# Uso (en PowerShell, desde la carpeta de este archivo):
#   1) ANTES de instalar Synapse:   powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase antes
#   2) Con Synapse instalado:       powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase synapse
#   3) Synapse DESINSTALADO y PC reiniciado (mejoras de audio de Windows):
#                                   powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase windows
#   4) Con Synapse y THX funcionando (donde guarda THX sus ajustes):
#                                   powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase thx
#   5) Con THX funcionando y Synapse CERRADO, en PowerShell como ADMINISTRADOR (como le llegan los ajustes a THX):
#                                   powershell -ExecutionPolicy Bypass -File .\capturar-synapse.ps1 -Fase thx-servicio
#
# Todo queda en el Escritorio, en la carpeta "rzr-captura" y en "rzr-captura.zip"
# ("rzr-captura-windows" / "rzr-captura-thx" / "rzr-captura-thx-servicio" y su .zip en esas fases).
# El script solo LEE el registro y copia los logs de Synapse: no modifica nada.

param(
    [ValidateSet('antes', 'synapse', 'windows', 'thx', 'thx-servicio')]
    [string]$Fase = 'synapse'
)

$ErrorActionPreference = 'Continue'
$carpeta = switch ($Fase) {
    'windows' { 'rzr-captura-windows' }
    'thx' { 'rzr-captura-thx' }
    'thx-servicio' { 'rzr-captura-thx-servicio' }
    default { 'rzr-captura' }
}
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

# Claves y carpetas donde THX podria guardar sus ajustes (fase thx).
$thxKeys = @(
    'HKLM\SOFTWARE\THX', 'HKLM\SOFTWARE\WOW6432Node\THX', 'HKCU\Software\THX',
    'HKLM\SOFTWARE\Razer', 'HKLM\SOFTWARE\WOW6432Node\Razer', 'HKCU\Software\Razer'
)
function Get-ThxFolders {
    foreach ($root in $env:ProgramData, $env:LOCALAPPDATA, $env:APPDATA, $env:ProgramFiles, ${env:ProgramFiles(x86)}) {
        if (-not $root -or -not (Test-Path $root)) { continue }
        Get-ChildItem $root -Directory -Depth 1 -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -match 'THX|Razer' } | ForEach-Object { $_.FullName }
    }
}
$thxFolders = if ($Fase -eq 'thx') { @(Get-ThxFolders) } else { @() }

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
    if ($Fase -ne 'thx') { return }
    foreach ($k in $thxKeys) {
        reg query $k /s 2>$null | Out-File $file -Append -Encoding utf8
    }
    # Archivos cambiados en el ultimo minuto: nombre, tamano y hora (sin contenido).
    "# archivos modificados recientemente" | Out-File $file -Append -Encoding utf8
    $desde = (Get-Date).AddMinutes(-1)
    foreach ($f in $thxFolders) {
        Get-ChildItem $f -Recurse -File -ErrorAction SilentlyContinue |
            Where-Object { $_.LastWriteTime -ge $desde } |
            ForEach-Object { "$($_.LastWriteTime.ToString('HH:mm:ss.fff'))  $($_.Length)  $($_.FullName)" } |
            Out-File $file -Append -Encoding utf8
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

# Fase thx-servicio: sin pasos guiados. Reune lo necesario para saber como le llegan los ajustes
# al efecto de THX: el servicio de THX, sus puertos, quien puede escribir los ajustes del
# dispositivo y los textos de ayuda de la utilidad de THX. Solo lee; no cambia nada.
if ($Fase -eq 'thx-servicio') {
    Write-Host ''
    Write-Host 'Reuniendo datos del servicio de THX (tarda un minuto)...' -ForegroundColor Green
    $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
    $info = Join-Path $out 'servicio.txt'
    "# administrador: $admin" | Out-File $info -Encoding utf8

    '# servicios de THX (por su ejecutable en el DriverStore)' | Out-File $info -Append -Encoding utf8
    Get-CimInstance Win32_Service -ErrorAction SilentlyContinue |
        Where-Object { $_.PathName -match 'thx|VSSrv|VSHelper' -or $_.Name -match 'THX|VSSrv' } |
        Format-List Name, DisplayName, State, StartMode, StartName, PathName | Out-File $info -Append -Encoding utf8

    '# procesos de THX' | Out-File $info -Append -Encoding utf8
    Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.ProcessName -match 'VSSrv|VSHelper|spatial|thx|Razer' } |
        Format-Table -AutoSize Id, ProcessName, Path | Out-File $info -Append -Encoding utf8

    '# puertos publicados por THX (HKLM\SOFTWARE\THX\Discovery) y quien escucha' | Out-File $info -Append -Encoding utf8
    $disc = Get-ItemProperty 'HKLM:\SOFTWARE\THX\Discovery' -ErrorAction SilentlyContinue
    if ($disc) {
        $disc.PSObject.Properties | Where-Object { "$($_.Value)" -match '^tcp://' } | ForEach-Object {
            $port = [int]("$($_.Value)" -replace '.*:', '')
            $conn = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1
            $proc = if ($conn) { (Get-Process -Id $conn.OwningProcess -ErrorAction SilentlyContinue).ProcessName } else { 'nadie' }
            "$($_.Name) = $($_.Value)  escucha: $proc" | Out-File $info -Append -Encoding utf8
        }
    }

    '# permisos de la clave de ajustes del dispositivo de salida' | Out-File $info -Append -Encoding utf8
    foreach ($k in @(Get-HeadsetEndpoints)) {
        $ps = 'Registry::' + $k + '\Properties'
        "## $k\Properties" | Out-File $info -Append -Encoding utf8
        (Get-Acl $ps -ErrorAction SilentlyContinue).AccessToString | Out-File $info -Append -Encoding utf8
    }

    '# ajustes de THX: dispositivo, usuario y archivos' | Out-File $info -Append -Encoding utf8
    Save-Snapshot 'thx-ajustes'
    reg query 'HKCU\Software\THX' /s 2>$null | Out-File (Join-Path $out 'thx-ajustes.txt') -Append -Encoding utf8
    reg query 'HKLM\SOFTWARE\THX' /s 2>$null | Out-File (Join-Path $out 'thx-ajustes.txt') -Append -Encoding utf8
    $thxData = Join-Path $env:ProgramData 'THX'
    if (Test-Path $thxData) {
        Get-ChildItem $thxData -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
            "$($_.LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss'))  $($_.Length)  $($_.FullName)" |
                Out-File $info -Append -Encoding utf8
            if ($_.Extension -match '^\.(json|conf|txt|log|ini|xml)$' -and $_.Length -lt 2MB) {
                $rel = $_.FullName.Substring($thxData.Length).TrimStart('\')
                $dest = Join-Path $out "programdata-thx\$rel"
                New-Item -ItemType Directory -Force (Split-Path $dest) | Out-Null
                Copy-Item $_.FullName $dest -ErrorAction SilentlyContinue
            }
        }
    }

    # Textos de las utilidades de THX (opciones de linea de comandos, mensajes del servicio).
    # Solo se leen los textos del archivo; no se ejecuta nada.
    $store = "$env:windir\System32\DriverStore\FileRepository"
    $exes = Get-ChildItem $store -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^thxrt' } |
        ForEach-Object { Get-ChildItem $_.FullName -File -Filter *.exe }
    foreach ($exe in $exes) {
        $file = Join-Path $out "textos-$($exe.BaseName).txt"
        "# $($exe.FullName)  $($exe.VersionInfo.FileVersion)" | Out-File $file -Encoding utf8
        $bytes = [IO.File]::ReadAllBytes($exe.FullName)
        $ascii = [Text.Encoding]::ASCII.GetString($bytes)
        $utf16 = [Text.Encoding]::Unicode.GetString($bytes)
        $patron = '(?i)usage|(^|\s)--?[a-z][a-z-]+|help|tcp://|zmq|json|preset|bass|spatial|drc|dialog|eq[_ ]?curve|sequence|endpoint|propert|registry|userstate|scope'
        foreach ($texto in $ascii, $utf16) {
            [regex]::Matches($texto, '[\x20-\x7E]{6,200}') | ForEach-Object { $_.Value } |
                Where-Object { $_ -match $patron } | Select-Object -Unique -First 1500 |
                Out-File $file -Append -Encoding utf8
        }
    }

    $zip = "$out.zip"
    if (Test-Path $zip) { Remove-Item $zip }
    Compress-Archive -Path "$out\*" -DestinationPath $zip
    Write-Host ''
    if (-not $admin) {
        Write-Host 'Aviso: no se ejecuto como administrador; faltaran algunos datos.' -ForegroundColor Yellow
    }
    Write-Host "Listo: $zip" -ForegroundColor Green
    Write-Host 'Enviame ese archivo.'
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
} elseif ($Fase -eq 'thx') {
    $pasos = @(
        'Pon musica. Abre Synapse en AUDIO con el headset conectado por el dongle y THX funcionando. Cierra rzr si esta abierto. No cambies nada todavia.',
        'SONIDO: activa THX SPATIAL AUDIO. (Si oyes el cambio, escribe si antes de Enter; si no, escribe no.)',
        'SONIDO: vuelve a ESTEREO. (Escribe si o no: oyes el cambio?)',
        'MEJORA: activa BASS BOOST y muevelo al maximo. (si/no)',
        'MEJORA: desactiva BASS BOOST.',
        'MEJORA: activa SOUND NORMALIZATION y muevela al maximo. (si/no)',
        'MEJORA: desactiva SOUND NORMALIZATION.',
        'MEJORA: activa VOICE CLARITY y muevela al maximo. (si/no)',
        'MEJORA: desactiva VOICE CLARITY.',
        'SONIDO: selecciona JUEGO / GAME y sube la banda de 1kHz al maximo. (si/no)',
        'SONIDO: vuelve a dejar JUEGO como estaba.',
        'Activa BASS BOOST al maximo otra vez y CIERRA Synapse por completo (icono junto al reloj > Salir / Exit). Se sigue oyendo el Bass Boost? (si/no)',
        'Abre el Administrador de tareas > Servicios, detiene los que empiecen por Razer o THX. Se sigue oyendo? (si/no; escribe tambien sus nombres)',
        'Reinicia los servicios (o reinicia el PC) y vuelve a abrir Synapse. Desactiva BASS BOOST.'
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
if ($Fase -eq 'thx') {
    Write-Host 'Captura guiada de THX Spatial Audio para rzr' -ForegroundColor Green
    Write-Host 'Haz cada cambio, espera 2 segundos y presiona Enter.'
    Write-Host 'Puedes escribir una respuesta o nota antes de Enter (s = saltar el paso).'
    Write-Host ''
    Save-Snapshot '00-thx-inicio'
    Log '00  THX funcionando, antes de los pasos'
} elseif ($Fase -eq 'windows') {
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
    if ($r) { Log "$n  RESPUESTA: $r" }
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

if ($Fase -eq 'thx') {
    Write-Host ''
    Write-Host 'Reuniendo datos del driver de THX...' -ForegroundColor Green
    $info = Join-Path $out 'thx-driver.txt'
    '# pnputil /enum-drivers (paquetes Razer / THX)' | Out-File $info -Encoding utf8
    $bloques = ((pnputil /enum-drivers) -join "`n") -split "`n\s*`n"
    $bloques | Where-Object { $_ -match 'Razer|THX' } | Out-File $info -Append -Encoding utf8
    '# servicios' | Out-File $info -Append -Encoding utf8
    Get-Service | Where-Object { $_.Name -match 'Razer|THX' -or $_.DisplayName -match 'Razer|THX' } |
        Format-Table -AutoSize Status, StartType, Name, DisplayName | Out-File $info -Append -Encoding utf8
    '# DLL cargadas en audiodg.exe (requiere PowerShell como administrador)' | Out-File $info -Append -Encoding utf8
    tasklist /m /fi 'imagename eq audiodg.exe' 2>&1 | Out-File $info -Append -Encoding utf8
    '# archivos en las carpetas THX/Razer del DriverStore' | Out-File $info -Append -Encoding utf8
    Get-ChildItem "$env:windir\System32\DriverStore\FileRepository" -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match 'thx|razer' } | ForEach-Object {
            Get-ChildItem $_.FullName -Recurse -File | ForEach-Object { "$($_.Length)  $($_.FullName)" }
        } | Out-File $info -Append -Encoding utf8
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
if ($Fase -eq 'thx') {
    Write-Host 'Enviame ese archivo. No desinstales Synapse todavia.'
} else {
    Write-Host 'Enviame ese archivo. Ya puedes desinstalar Synapse.'
}

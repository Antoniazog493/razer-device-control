# Handoff: sesión local en la PC con Windows

Para una sesión de Claude Code que corre **en la PC del usuario**, con Windows, el dongle conectado y THX instalado. Complementa a [ESTADO.md](ESTADO.md), que sigue siendo la fuente de verdad. Este archivo explica qué cambia al trabajar en local y por dónde seguir.

Última actualización: 2026-09-26 · rama `UI-creation`.

## Por qué en local

Hasta ahora el trabajo se hacía en la nube: se escribía un script, el usuario lo corría, mandaba un `.zip` y se analizaba. En local se puede ir directo a la fuente: leer el registro, ver servicios y puertos, correr `rzr.exe` con el headset real y repetir una prueba en segundos. Lo que sigue no cambia: **el usuario es quien escucha**. Cualquier prueba de sonido termina en una pregunta al usuario, igual que la prueba guiada.

## Preparar el entorno

1. `git fetch origin` y `git checkout UI-creation` y `git pull`.
2. Rust estable con el toolchain MSVC (`rustup default stable-x86_64-pc-windows-msvc`; necesita las Build Tools de Visual Studio con "Desarrollo para el escritorio con C++"). Es lo mismo que usa el CI (`windows-latest`). **En esta PC ya están instalados** (2026-09-26, con `winget`). Si `cargo` no aparece en una consola abierta antes de la instalación, agrega `%USERPROFILE%\.cargo\bin` al `PATH` de esa consola.
3. Node.js, solo para `node --check`.
4. Comprobar que todo pasa antes de tocar nada:

   ```powershell
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   node --check ui/app.js; node --check ui/demo.js
   cargo build --release
   ```

   En Windows no hace falta el target `x86_64-pc-windows-gnu`: `cargo build --release` ya es la compilación de Windows.
5. Para probar el panel: `cargo run --release` (headset real) o `cargo run -- --demo`. **Cerrar Synapse** antes de usar rzr con el headset: los dos se pelean por el dongle.
6. Para ver qué muestra el panel sin pedirle al usuario que lo describa, se puede capturar **solo la ventana de rzr** con `PrintWindow` (flag `PW_RENDERFULLCONTENT` = 2, funciona con WebView2 aunque la ventana esté tapada). La imagen se guarda en la carpeta temporal y no se sube.

## Reglas extra al trabajar en la PC real

Todas las de [REGLAS.md](REGLAS.md) siguen valiendo. Además:

- **Headset:** solo secuencias ya verificadas (ver `device.rs` y [ADR 0002](adr/0002-secuencias-verificadas.md)). Nada de mandar comandos "a ver qué pasa".
- **Registro de Windows:**
  - Leer es libre.
  - Antes de **escribir**, hay que respaldar la clave con `reg export "<clave>" respaldo.reg`, decirle al usuario qué valor se va a cambiar y **pedirle permiso**. Después se restaura el valor, o se deja como estaba si la prueba lo cambió.
  - Nunca se tocan claves fuera de las de THX o de la salida de los audífonos.
- **Drivers y servicios:**
  - No instalar, desinstalar ni reiniciar drivers.
  - Detener un servicio solo con permiso, y volver a iniciarlo al terminar.
  - No usar `pnputil /delete-driver`.
- **Archivos de Razer/THX:** se pueden leer, sacar textos (`strings`) y ejecutar con `--help` para ver sus opciones. **Nunca se copian al repo.** Tampoco se suben capturas, `debug.log` ni rutas con el nombre del usuario.
- **Administrador:** si algo requiere PowerShell como administrador, se le pide al usuario que lo abra. No se busca cómo saltarse los permisos.
- **Commits:** en inglés, en la rama `UI-creation`, y con ESTADO.md al día en el mismo commit (ver [REGLAS.md §6](REGLAS.md#6-documentación)).

## Lo que ya se sabe de THX (resumen)

El detalle y las fuentes están en [HALLAZGOS.md › THX](HALLAZGOS.md#thx-spatial-audio).

- **Dónde está el estado de THX:** es un JSON en `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{id}\Properties`, valor `{d5e8f0ab-4de6-4d91-ab21-68868dda6a4a},6`. El `{id}` es la salida "Speakers (Razer BlackShark V2 Pro)"; en esta PC es `{68854153-5b36-4efa-b61a-6ffb850dd6fa}`.
  - El valor `,7` guarda lo mismo en protobuf.
  - Hay copias por preset de THX en `HKCU\Software\THX\SpatialAudio\UserState`.
- **Campos del JSON:**

  | Opción | Campos |
  |---|---|
  | Espacial | `spatialEnabled` |
  | Bass Boost | `bassBoostEnabled`, `bassBoost` |
  | Normalización | `drcEnabled`, `drcLevel` |
  | Claridad de voz | `dialogEnhancementEnabled`, `dialogEnhancement` |
  | Preset de THX | `presetName` |
  | Curva | `eqCurve`: 31 valores, un 0 y luego las 10 bandas repetidas 3 veces |

  `sequenceNumber` sube en cada cambio.
- **Cómo se cambian los ajustes:** los guarda el **servicio de THX** (`VSSrv`, `C:\Windows\System32\VSSrv.exe`, corre como `LocalSystem` y sigue activo sin Synapse). Tiene dos entradas:
  - **COM** (`VSSrv.CVSSrvTHXSettings`, interfaz `IVSSrvTHXSettings`): lo que usa rzr (`src/thx.rs`, [ADR 0004](adr/0004-thx-por-com.md)). Solo tiene interruptores para Spatial, Bass Boost y Claridad de voz, y el nivel de DRC (que no activa la normalización).
  - **ZeroMQ** (`tcp://127.0.0.1:49671`, un `ROUTER`; `:49670` es un `PUB`): lo que usa Synapse, con `ThxV3Native` → `thxv3lib`. Permitiría todo, pero el servicio no respondió a los mensajes armados según lo averiguado.
- **Leer `,6` por `IPropertyStore` corta el texto en 259 caracteres;** rzr lo lee con `winreg`.
- **Los presets de THX no son los del headset:** THX tiene su propia curva por software, que se suma a la del headset. Con Synapse abierto, el botón EQ del headset también cambia la curva de THX.

## Qué sigue, en orden

Hecho el 2026-09-26: reconocimiento del servicio, estado de THX en MEJORAS e interruptores por COM (✅ en el headset para Claridad de voz y Spatial). Ver ESTADO.md.

### 1. Normalización y niveles de THX por ZeroMQ

1. **Capturar a Synapse.** Instalar Wireshark con Npcap (lo instala el usuario; pídeselo) y capturar la interfaz "Adapter for loopback traffic capture" con el filtro `tcp.port == 49671`, mientras el usuario activa la normalización en Synapse y mueve su nivel.
2. **Comparar** esos mensajes con lo que se intentó (HALLAZGOS: partes `x-originator:` y `x-payload:`, `THXMessage` con `Any`, `Register {pid}` y luego `State` completo con `sequence_number` + 1). Mirar el orden de las partes, las identidades del socket y si hay un saludo previo.
3. **Probar lo mismo desde un script** (con permiso, con Synapse cerrado): primero algo reversible y audible, confirmando en el JSON. Después pasarlo a `src/thx.rs` y un ADR si cambia el camino.

### 2. Pendientes de ESTADO.md que ahora son rápidos

- **Terminar de revisar el panel en Windows:** parpadeo blanco al abrir, arrastrar un `.synapse4`, sliders y EQ.
- **Ronda 2 de la prueba guiada** (AJUSTES › DIAGNÓSTICO), anotando el preset de THX que muestra MEJORAS, porque su curva se suma a la del headset. Si hace falta aislar el headset, pon el preset `Custom` plano de THX o desactiva las mejoras de audio. Pregunta antes de cambiar cualquiera de las dos cosas.
- **¿THX suena con `VSSrv` detenido?** Requiere PowerShell como administrador y permiso. Vuelve a iniciarlo al terminar.
## Al terminar la sesión

- ESTADO.md al día: qué quedó ✅ en hardware, qué sigue ⏳.
- Todas las comprobaciones en verde y `git push -u origin UI-creation`.
- Si queda algo a medias, actualiza este archivo (o bórralo si ya no aporta nada que no diga ESTADO.md).

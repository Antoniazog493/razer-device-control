# Handoff: sesión local en la PC con Windows

Para una sesión de Claude Code que corre **en la PC del usuario**, con Windows, el dongle conectado y THX instalado. Complementa a [ESTADO.md](ESTADO.md), que sigue siendo la fuente de verdad. Este archivo explica qué cambia al trabajar en local y por dónde seguir.

Última actualización: 2026-09-26 · rama `UI-creation`.

## Por qué en local

Hasta ahora el trabajo se hacía en la nube: se escribía un script, el usuario lo corría, mandaba un `.zip` y se analizaba. En local se puede ir directo a la fuente: leer el registro, ver servicios y puertos, correr `rzr.exe` con el headset real y repetir una prueba en segundos. Lo que sigue no cambia: **el usuario es quien escucha**. Cualquier prueba de sonido termina en una pregunta al usuario, igual que la prueba guiada.

## Preparar el entorno

1. `git fetch origin` y `git checkout UI-creation` y `git pull`.
2. Rust estable con el toolchain MSVC (`rustup default stable-x86_64-pc-windows-msvc`; necesita las Build Tools de Visual Studio con "Desarrollo para el escritorio con C++"). Es lo mismo que usa el CI (`windows-latest`).
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
- **Cómo cambia Synapse los ajustes:** no escribe el registro. Usa `ThxV3Native` → `thxv3lib` → ZeroMQ → **servicio de THX** (`VSSrv.exe`, paquete `thxrtsvc.inf`). Las direcciones del servicio están en `HKLM\SOFTWARE\THX\Discovery` (`tcp://127.0.0.1:49671` y `:49670`).
- **Sin Synapse:** el Bass Boost siguió sonando con Synapse cerrado y los servicios de Razer detenidos. No se sabe si el servicio de THX seguía corriendo.
- **Los presets de THX no son los del headset:** THX tiene su propia curva por software, que se suma a la del headset. Con Synapse abierto, el botón EQ del headset también cambia la curva de THX.

## Qué sigue, en orden

### 1. Averiguar cómo escribir los ajustes de THX

Es lo que falta para el punto 3 de "Sigue" en ESTADO.md. Una sola sesión local puede resolverlo.

1. **Reconocimiento (solo lectura).** Corre `tools/capturar-synapse.ps1 -Fase thx-servicio` como administrador, o haz lo mismo a mano:
   - servicio de `VSSrv.exe`: nombre, cuenta con la que corre e inicio;
   - quién escucha en los puertos de `Discovery`;
   - permisos (ACL) de la clave `Properties`;
   - contenido de `C:\ProgramData\THX\usb\1532\0555\thx_spatial.conf.json`;
   - textos de `spatial-config-util.exe` y `VSSrv.exe`.
2. **`spatial-config-util.exe`** (en `DriverStore\FileRepository\thxrtscu.inf_*`). Si sus textos muestran opciones de línea de comandos, prueba primero `--help` o `-h` y enséñale el resultado al usuario. Si permite fijar Bass Boost y los demás ajustes, es el camino más limpio.
3. **Escribir el registro** (con respaldo y permiso):
   - con música sonando y Synapse cerrado, cambia `bassBoostEnabled` en `,6` y sube `sequenceNumber`;
   - pregúntale al usuario si oye el cambio;
   - si no se oye, prueba también `,7` (el protobuf: el campo 1 es `sequenceNumber`, el 2 es `spatialEnabled`, etc.);
   - comprueba si hace falta reiniciar el audio (`Restart-Service audiosrv`, con permiso).
4. **Hablarle al servicio como Synapse.** Captura el tráfico de loopback (Wireshark con Npcap, interfaz "Adapter for loopback traffic capture") mientras el usuario cambia Bass Boost en Synapse. Los mensajes son ZeroMQ (ZMTP 3); probablemente llevan el mismo protobuf que `,7`.
5. **¿Hace falta el servicio?** Repite la prueba que funcione con `VSSrv` detenido (con permiso, y vuelve a iniciarlo al terminar).
6. **Anota el resultado** en HALLAZGOS.md y ADR 0003 (el camino elegido y por qué), y actualiza ESTADO.md.

### 2. Mostrar el estado de THX en rzr (solo lectura)

Ya se puede hacer sin esperar al paso 1:

- Un módulo nuevo, por ejemplo `src/thx.rs`, que:
  - busque la salida del headset (ya hay código de endpoints en `winaudio.rs`);
  - lea `{d5e8f0ab-…},6` con `IPropertyStore` o con `winreg`;
  - lo convierta con `serde` en una estructura con los campos de arriba.
- La pestaña MEJORAS debe mostrar qué está activado y el preset de THX. Si no hay THX instalado, muestra un aviso y no falla.
- Pruebas unitarias del análisis del JSON: usa un ejemplo **escrito a mano** con la forma del JSON de HALLAZGOS, no una captura real.
- Actualiza ARQUITECTURA.md (módulo nuevo), `demo.js` (estado nuevo) y CONTEXT.md si aparece un término.

### 3. Escribir los ajustes de THX

Con el camino elegido en el paso 1: activar y desactivar Bass Boost, Normalización, Claridad de voz y Espacial, y confirmar cada cambio releyendo el JSON. Esto va con un ADR si el camino es difícil de revertir (por ejemplo, depender del protocolo del servicio).

### 4. Pendientes de ESTADO.md que ahora son rápidos

- **Abrir el panel nuevo en Windows** y corregir lo que aparezca.
- **Ronda 2 de la prueba guiada** (AJUSTES › DIAGNÓSTICO), anotando qué preset de THX está activo y su curva, porque se suma a la del headset. Si hace falta aislar el headset, pon el preset `Custom` plano de THX o desactiva las mejoras de audio. Pregunta antes de cambiar cualquiera de las dos cosas.

## Al terminar la sesión

- ESTADO.md al día: qué quedó ✅ en hardware, qué sigue ⏳.
- Todas las comprobaciones en verde y `git push -u origin UI-creation`.
- Si queda algo a medias, actualiza este archivo (o bórralo si ya no aporta nada que no diga ESTADO.md).

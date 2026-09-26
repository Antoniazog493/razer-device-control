# Estado del proyecto

Fuente de verdad del trabajo: qué está hecho, qué falta y qué sigue. **Se actualiza en cada cambio**, en el mismo commit.

Última actualización: 2026-09-26 · rama `UI-creation` · versión 0.2.0 (sin publicar)

Leyenda de verificación:

- ✅ **Verificado en el headset real**
- 🟡 **Implementado, sin confirmar en hardware** (probado en demo, CI o con pruebas unitarias)
- ❌ **No funciona** (confirmado)
- ⏳ **Esperando una prueba del usuario**

## Hecho

### Headset

| Función | Estado | Notas |
|---|---|---|
| Leer batería, carga, enlace, preset activo | ✅ | |
| Eventos del headset (botón EQ, enlace, silencio) | ✅ | El botón EQ envía 07 → 09 → 08 → FF. |
| Elegir Juego / Película / Música | ✅ | El cambio se oye. |
| Curva Personalizada | ❌ ⏳ | Se guarda y se lee igual, pero **no se oye**. La ronda 2 de la prueba guiada busca por qué. |
| Presets Esports (elegir y escribir su curva) | 🟡 ⏳ | Incluido en la ronda 2. |
| Sidetone, No molestar, Apagado automático | 🟡 | Comandos verificados por OpenRazer en este modelo; no confirmado en este headset. |
| Firmware, número de serie, firmware del dongle | 🟡 | |
| Registro de caídas del enlace | 🟡 | |

### Windows y la app

| Función | Estado | Notas |
|---|---|---|
| Volumen y silencio de Windows | 🟡 | |
| Dispositivo predeterminado al conectar | 🟡 | |
| Perfiles, importar `.synapse4` (archivo o arrastrando) | 🟡 | El importador tiene pruebas con un perfil real. |
| Iniciar con Windows / proceso en segundo plano | 🟡 | |
| Panel nuevo (WebView2) | 🟡 ⏳ | Abre en Windows 11 y aplica el perfil; MEJORAS revisada. Falta revisar: parpadeo blanco al abrir, arrastrar un `.synapse4`, sliders y EQ. |
| Estado de THX en MEJORAS (interruptores, niveles, preset de THX) | ✅ | Coincide con el JSON del registro; se actualiza cada 2 s. |
| THX: Claridad de voz y THX Spatial Audio (activar/desactivar) | ✅ | Con Synapse cerrado. Claridad de voz se oye; Spatial se notó poco con música (por COM directo sí se oyó claro). Cada cambio se confirma en el JSON en menos de 1 s. |
| THX: Bass Boost (activar/desactivar) | 🟡 | El cambio queda confirmado en el estado de THX; falta confirmar de oído. |
| THX: Normalización y niveles | — | Solo se muestran. Por COM no se pueden cambiar (ver "Sigue"). |
| Prueba guiada del EQ (ronda 2) | 🟡 ⏳ | |
| Registro de depuración (`debug.log`) | ✅ | Registra también los cambios de THX. |

### Investigación

- Qué hace Synapse con cada opción (captura): ✅ ver [HALLAZGOS.md](HALLAZGOS.md).
- THX es un efecto de audio de Windows, no del headset: ✅.
- Dónde guarda THX sus ajustes: ✅ en el registro de la salida de los audífonos (JSON en `{d5e8f0ab-…},6`, protobuf en `,7`) y en `HKCU\Software\THX`. Qué campo cambia cada opción: ✅. Ver [HALLAZGOS.md](HALLAZGOS.md#dónde-guarda-thx-sus-ajustes).
- Cómo le llegan los ajustes: ✅ los guarda el servicio de THX (`VSSrv`, `LocalSystem`, sin Synapse). Tiene dos entradas: ZeroMQ (lo que usa Synapse) y una **interfaz COM** (lo que usa rzr, [ADR 0004](adr/0004-thx-por-com.md)). `spatial-config-util.exe` no sirve para esto. Ver [HALLAZGOS.md](HALLAZGOS.md#cómo-le-llegan-los-ajustes-a-thx).
- Bass Boost de THX sigue sonando con Synapse cerrado y los servicios de Razer detenidos: ✅ (de oído, captura `-Fase thx`).
- Mejoras de micrófono con THX instalado: van al motor THX (`SetCapture…`) ✅; dónde se guardan: sin averiguar.
- Claves de registro de las mejoras de Windows (Bass Boost, Loudness): ✅ capturadas; no implementadas.

## Esperando al usuario (en la PC)

1. **Terminar de revisar el panel en Windows** (`cargo run --release`, o el artefacto `rzr-windows` del último build): que abra sin parpadeo blanco, arrastrar un `.synapse4`, sliders de volumen y sidetone, y la curva del EQ.
2. **Ronda 2 de la prueba guiada** (AJUSTES › DIAGNÓSTICO) y enviar `debug.log`. Cerrar Synapse antes (se pelean por el dongle). Anotar el preset de THX que muestra MEJORAS, porque su curva se suma a la del headset.
3. **Confirmar de oído el Bass Boost de THX** desde MEJORAS, con música.

## Sigue (en orden)

1. **Hacer que la curva Personalizada se oiga.** Depende del resultado de la ronda 2. Si funciona "cambiar de preset y volver", dejarlo como método por defecto; si no, analizar `debug.log` contra la secuencia de OpenRazer ([HALLAZGOS.md](HALLAZGOS.md#ecualizador)).
2. **Confirmar el panel nuevo en Windows** y corregir lo que aparezca.
3. **Normalización y niveles de THX.** Por COM no se puede; hace falta el camino de Synapse (ZeroMQ, enviar un `thx.sa.State` completo). El servicio no respondió a los mensajes armados según lo averiguado. Siguiente paso: capturar el tráfico de loopback con Wireshark + Npcap mientras Synapse cambia la normalización, y comparar con lo que se envió ([HALLAZGOS.md](HALLAZGOS.md#cómo-le-llegan-los-ajustes-a-thx)).
4. **Revisar la configuración de audio de Windows:** avisar si las mejoras de audio están desactivadas o Windows Sonic encendido, y ofrecer corregirlo.
5. **Mejoras de Windows sin Synapse** (Bass Boost, Loudness) con las claves ya capturadas, para quien no tenga THX.
6. **Confirmar en hardware** lo marcado 🟡 y actualizar esta tabla.
7. **Publicar la versión 0.2.0** (release con `rzr.exe`) cuando 1 y 2 estén listos.

## Ideas (sin prioridad)

- Guardar los ajustes de THX en el perfil y aplicarlos al conectar (hoy son ajustes de Windows, fuera del perfil).
- Ícono en la bandeja del sistema para el proceso en segundo plano (batería, preset, abrir el panel).
- Otros modelos (BlackShark V3, HyperSpeed): OpenRazer los describe con una ficha por modelo; se podría hacer igual.
- Mejoras de micrófono con Equalizer APO o RNNoise.
- Aviso de batería baja.

## Deuda técnica

- Solo hay pruebas unitarias; el camino página ↔ Rust no tiene prueba automática (se probó a mano con Xvfb). Una prueba con Playwright sobre `ui/` + `demo.js` cubriría la página.
- `demo.js` duplica a mano la forma del estado de `App::view`; si divergen, la demo miente. Se podría generar un estado de ejemplo desde Rust.
- El dispositivo predeterminado se cambia con PowerShell (interfaz no documentada de Windows).
- La interfaz COM de THX se declara a mano según la biblioteca de tipos de `VSSrv` 3.2.3.0; si THX la cambia, hay que revisarla ([ADR 0004](adr/0004-thx-por-com.md)).

## Preguntas abiertas

- ¿El headset necesita que el modo remoto quede encendido para que el EQ se oiga?
- ¿Qué hace realmente `0x9E` (Speaker Preset EQ Status)? En la ronda 1 no cambió nada.
- ¿THX sigue sonando si se detiene el servicio de THX (`VSSrv`)? Detenerlo requiere administrador y permiso del usuario.
- ¿Por qué el servicio de THX no responde por ZeroMQ a los mensajes armados como los de Synapse? ¿Qué son los parámetros 1–3 de `GetParam`?
- La curva de THX se suma a la del headset (y Synapse la cambia al pulsar el botón EQ). ¿Influyó en la ronda 1 de la prueba guiada? Conviene anotar el preset de THX activo al repetirla.

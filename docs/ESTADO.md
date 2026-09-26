# Estado del proyecto

Fuente de verdad del trabajo: qué está hecho, qué falta y qué sigue. **Se actualiza en cada cambio**, en el mismo commit.

Última actualización: 2026-09-26 · rama `UI-creation` · versión 0.2.0 (sin publicar)

**Objetivo:** que rzr reemplace a Synapse por completo, con THX funcionando **sin Synapse instalado**. Desinstalar Synapse también desinstala THX, así que THX se reinstalará solo (ver "Sigue").

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
| Curva Personalizada (en el headset) | ❌ | Se guarda y se lee igual, pero **no se oye** (confirmado otra vez el 2026-09-26). El EQ que se oye en Synapse es el de THX: ver "EQ de THX" abajo. |
| Presets Esports (elegir y escribir su curva) | 🟡 ⏳ | Incluido en la ronda 2. |
| Sidetone | ❌ | El headset confirma el encendido y el nivel (lo lee de vuelta), pero **no se oye**. Con Synapse sí se oye: además lo activa en THX (ver "Sigue" 3). |
| No molestar, Apagado automático | 🟡 | Comandos verificados por OpenRazer en este modelo; no confirmado en este headset. |
| Firmware, número de serie, firmware del dongle | 🟡 | |
| Registro de caídas del enlace | 🟡 | Una caída real (2026-09-26) quedó registrada, pero ~6 s tarde y con 1,6 s de duración en lugar de ~7,5 s: se perdía el aviso del headset. Corregido (el aviso se lee siempre); falta verlo en la próxima caída. |

### Windows y la app

| Función | Estado | Notas |
|---|---|---|
| Volumen de Windows | ✅ | |
| Silencio de Windows | 🟡 | |
| Dispositivo predeterminado al conectar | 🟡 | |
| Perfiles, importar `.synapse4` (archivo o arrastrando) | ✅ | Arrastrar a la ventana funciona. |
| Iniciar con Windows / proceso en segundo plano | 🟡 | |
| Panel nuevo (WebView2) | ✅ | Abre bien en Windows 11, aplica el perfil, sliders y curva se mueven bien. Lo que no suena (sidetone, curva) es del headset, no del panel. |
| Estado de THX en MEJORAS (interruptores, niveles, preset de THX) | ✅ | Coincide con el JSON del registro; se actualiza cada 2 s. |
| THX: Claridad de voz y THX Spatial Audio (activar/desactivar) | ✅ | Con Synapse cerrado. Claridad de voz se oye; Spatial se notó poco con música (por COM directo sí se oyó claro). Cada cambio se confirma en el JSON en menos de 1 s. |
| THX: Bass Boost (activar/desactivar) | ❌ | El cambio queda confirmado en el estado de THX, pero **no se oye** con el nivel en 50 (2026-09-26). Con Synapse, al 100, sí se oía: falta poder cambiar el nivel. |
| THX: Normalización y niveles | — | Solo se muestran en rzr. Por ZeroMQ sí se pueden cambiar (✅ oído con un script, ver "Sigue" 1). |
| EQ de THX (curva por software, `SetCurrentModeEQGains`) | ✅ | Probado con un script por COM, aún **no está en rzr**: una curva de "teléfono" se oyó al instante y se restauró la curva Música. Es el EQ que Synapse cambia al editar Juego/Película/Música. |
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

1. **Ronda 2 de la prueba guiada** (AJUSTES › DIAGNÓSTICO) y enviar `debug.log`. Ya no es urgente: el EQ que se oye va por THX (ver "Sigue" 1). Sirve para saber si la curva del headset puede funcionar sin THX.

## Sigue (en orden)

El sondeo de Synapse del 2026-09-26 mostró qué hace Synapse con THX instalado (todo se oyó). rzr lo imitará ([HALLAZGOS.md › Ecualizador](HALLAZGOS.md#ecualizador), [Micrófono](HALLAZGOS.md#micrófono)).

1. **Niveles de THX por ZeroMQ** (Bass Boost, Claridad de voz, Normalización): ✅ verificado de oído con un script que imita a Synapse (el nivel de Bass Boost 0 ↔ 100 se oyó; [HALLAZGOS.md](HALLAZGOS.md#cómo-le-llegan-los-ajustes-a-thx)). Falta pasarlo a `src/thx.rs` (cliente ZeroMQ: `Register` y `State` completo con la secuencia siguiente) con un ADR que actualice el [ADR 0004](adr/0004-thx-por-com.md), y agregar los sliders de nivel en MEJORAS.
2. **EQ como Synapse:** Juego/Película/Música eligen el preset del headset **y** el de THX con su curva; Personalizado escribe la curva en el headset y en el preset `Custom` de THX. Registrar la decisión en un ADR.
3. **Sidetone como Synapse:** además de `0x98`/`0x99`, fijar el nivel en THX (`IVSSrvSettings::SetInputSidetoneLevel`, nivel = slider × 31,62 / 10 000) y **activarlo**: Synapse lo hace con `SetStartCaptureStatus`, que no se ve en `IVSSrvSettings`. Primero probar de oído si basta con `SetInputSidetoneState(1)`; si no, abrir una captura del micrófono por WASAPI mientras el sidetone esté encendido (hipótesis en [HALLAZGOS.md](HALLAZGOS.md#micrófono)).
4. **Micrófono por THX** (`IVSSrvSettings`, parámetros ya identificados en [HALLAZGOS.md](HALLAZGOS.md#micrófono)): EQ con presets (Default, MicBoost, Broadcast, Conference y Personalizado de −12 a +12, con `0x96` al headset), normalización (14/15), claridad de voz (10/11), reducción de ruido (6/7) y puerta de voz (2/3, en dB). Probar de oído grabando o con el sidetone (aunque el de Synapse no pasa por las mejoras).
5. **Separar THX de Synapse.** Respaldar los instaladores de THX de `C:\ProgramData\Package Cache` (fuera del repo), desinstalar Synapse, reinstalar solo THX y confirmar que el efecto, `VSSrv` y rzr siguen funcionando ([HALLAZGOS.md](HALLAZGOS.md#paquete-de-driver)). Ya no hace falta Synapse para sondear; conviene hacerlo cuando 1 a 4 funcionen en rzr, por si hay que volver a comparar.
6. **Revisar la configuración de audio de Windows:** avisar si las mejoras de audio están desactivadas o Windows Sonic encendido, y ofrecer corregirlo.
7. **Confirmar en hardware** lo marcado 🟡 y actualizar esta tabla.
8. **Interfaz nativa y liviana** en lugar de WebView2 (preferencia del usuario): WebView2 deja ~30 MB de caché en `%LOCALAPPDATA%\rzr\webview`. Se hará cuando lo anterior funcione; requiere un ADR que reemplace al [0001](adr/0001-interfaz-web-webview2.md).
9. **Mejoras de Windows sin Synapse** (Bass Boost, Loudness) con las claves ya capturadas, para quien no tenga THX.
10. **Publicar la versión 0.2.0** (release con `rzr.exe`) cuando 1 a 5 estén listos.

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

- ¿El headset necesita que el modo remoto quede encendido para que el EQ se oiga? ¿O su curva Personalizada solo se oye sin THX instalado?
- ¿El Bass Boost de THX al nivel 50 es demasiado sutil, o no se aplica? Se aclara al poder cambiar el nivel.
- ¿Qué hace realmente `0x9E` (Speaker Preset EQ Status)? En la ronda 1 no cambió nada.
- ¿THX sigue sonando si se detiene el servicio de THX (`VSSrv`)? Detenerlo requiere administrador y permiso del usuario.
- ¿Por qué el servicio de THX no responde por ZeroMQ a los mensajes armados como los de Synapse? ¿Qué son los parámetros 1–3 de `GetParam`?
- La curva de THX se suma a la del headset (y Synapse la cambia al pulsar el botón EQ). ¿Influyó en la ronda 1 de la prueba guiada? Conviene anotar el preset de THX activo al repetirla.

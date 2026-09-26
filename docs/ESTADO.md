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
| Eventos del headset (botón EQ, enlace, silencio) | ✅ | Pulsación corta: siguiente preset de la familia; larga: cambia de familia ([HALLAZGOS](HALLAZGOS.md#headset-y-protocolo)). El panel lo sigue en menos de 1 s, también justo después de un cambio desde la PC (antes se ignoraba 4 s y salía un aviso rojo; corregido y probado el 2026-09-26). |
| Elegir Juego / Película / Música | ✅ | El cambio se oye. |
| Curva Personalizada (en el headset) | ❌ | Se guarda y se lee igual, pero **no se oye** (confirmado otra vez el 2026-09-26). Con THX instalado no hace falta: rzr aplica la misma curva en THX y esa sí se oye (ver "EQ como Synapse" abajo). |
| Presets Esports (elegir y escribir su curva) | 🟡 ⏳ | En el headset, incluido en la ronda 2. Con THX se oyen (ver "EQ como Synapse"). |
| Sidetone | ✅ | Oído el 2026-09-26 con Synapse cerrado, **solo con los comandos del headset** (`0x98`/`0x99`; el sidetone de THX seguía apagado): se enciende, se apaga y el volumen cambia (20 ↔ 100). Antes ese mismo día no se había oído (ver "Preguntas abiertas"). |
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
| THX: Bass Boost (activar/desactivar) | ✅ | Por COM. Al 50 casi no se notaba; con el nivel en 100 se oye. |
| THX: niveles de Bass Boost y Claridad de voz | ✅ | Por ZeroMQ ([ADR 0005](adr/0005-thx-por-zeromq.md)), con Synapse cerrado. Se oyó 0 ↔ 100 en los dos (2026-09-26); cada cambio se confirma en el JSON en menos de 1 s. |
| THX: Normalización (activar/desactivar y nivel) | 🟡 ⏳ | Por ZeroMQ, igual que Synapse. El servicio la guarda (confirmado en el JSON), pero con música **no se notó**, ni desde rzr ni desde un script (prueba a ciegas del 2026-09-26: de tres cambios solo se notó el de Spatial). Falta una prueba mejor (ver "Esperando al usuario"). |
| EQ como Synapse: el preset del headset elige también el preset y la curva de THX | ✅ | [ADR 0006](adr/0006-eq-como-synapse.md). Oído el 2026-09-26: Juego/Película/Música, los cinco Esports (THX `Custom` con su curva) y la curva Personalizada al arrastrarla; cada cambio queda en THX en menos de 1 s. Al cambiar Spatial, rzr vuelve a poner la curva (THX la cambiaba por otra plana). Con el botón EQ del headset y el panel abierto: ✅ oído el 2026-09-26 (presets Estándar y Esports, en menos de 1 s). **Con el panel cerrado, el proceso en segundo plano no sigue al botón:** THX se queda con la curva anterior (ver "Sigue"). |
| Prueba guiada del EQ (ronda 2) | 🟡 ⏳ | |
| Mejoras del micrófono por THX (EQ con presets y Personalizado, normalización, claridad vocal, reducción de ruido, puerta de voz) | ✅ | [ADR 0007](adr/0007-microfono-en-el-perfil.md). Oído el 2026-09-26 con Synapse cerrado, escuchando el micrófono en vivo: todas cambian y sus niveles también. Se guardan en el perfil; cada cambio se confirma leyendo el servicio. |
| Preset de EQ del micrófono en el headset (`0x96`) | 🟡 | Mismos bytes que Synapse; se envía con el perfil. Su efecto no se nota aparte (el EQ que se oye es el de THX). |
| Volver a poner las mejoras del micrófono tras reiniciar la PC | 🟡 ⏳ | El proceso en segundo plano las manda a THX al conectarse a su servicio. Falta la prueba (ver "Esperando al usuario"). |
| Registro de depuración (`debug.log`) | ✅ | Registra también los cambios de THX. |

### Investigación

- Qué hace Synapse con cada opción (captura): ✅ ver [HALLAZGOS.md](HALLAZGOS.md).
- THX es un efecto de audio de Windows, no del headset: ✅.
- Dónde guarda THX sus ajustes: ✅ en el registro de la salida de los audífonos (JSON en `{d5e8f0ab-…},6`, protobuf en `,7`) y en `HKCU\Software\THX`. Qué campo cambia cada opción: ✅. Ver [HALLAZGOS.md](HALLAZGOS.md#dónde-guarda-thx-sus-ajustes).
- Cómo le llegan los ajustes: ✅ los guarda el servicio de THX (`VSSrv`, `LocalSystem`, sin Synapse). Tiene dos entradas, y rzr usa las dos: una **interfaz COM** ([ADR 0004](adr/0004-thx-por-com.md)) y ZeroMQ, lo que usa Synapse ([ADR 0005](adr/0005-thx-por-zeromq.md)). `spatial-config-util.exe` no sirve para esto. Ver [HALLAZGOS.md](HALLAZGOS.md#cómo-le-llegan-los-ajustes-a-thx).
- Bass Boost de THX sigue sonando con Synapse cerrado y los servicios de Razer detenidos: ✅ (de oído, captura `-Fase thx`).
- Mejoras de micrófono con THX instalado: van al motor THX por `IVSSrvSettings` ✅. Dónde las guarda THX: no se encontró (ni en el registro de THX ni en el del micrófono); probablemente solo en memoria ([ADR 0007](adr/0007-microfono-en-el-perfil.md)).
- Claves de registro de las mejoras de Windows (Bass Boost, Loudness): ✅ capturadas; no implementadas.

## Esperando al usuario (en la PC)

1. **Mejoras del micrófono tras reiniciar:** con "Iniciar con Windows" activado y alguna mejora encendida en el perfil (por ejemplo la reducción de ruido), reiniciar la PC **sin abrir el panel ni Synapse** y escucharse en vivo: la mejora debería seguir activa. Si no, abrir el panel y comprobar que vuelve (eso confirmaría que THX las olvida y que falla el proceso en segundo plano).
2. **Normalización de THX con un audio de mucho contraste:** un video o una película con partes muy bajas (susurros) y muy fuertes (explosiones). En MEJORAS, con la normalización al 100, apagarla y encenderla: con ella encendida, lo bajo debería sonar más fuerte y lo fuerte, más suave. Si no se nota, probar lo mismo desde Synapse para saber si es cosa de rzr.
3. **Ronda 2 de la prueba guiada** (AJUSTES › DIAGNÓSTICO) y enviar `debug.log`. Ya no es urgente: el EQ que se oye va por THX (ver "Sigue" 1). Sirve para saber si la curva del headset puede funcionar sin THX.

## Sigue (en orden)

El sondeo de Synapse del 2026-09-26 mostró qué hace Synapse con THX instalado (todo se oyó). rzr lo imitará ([HALLAZGOS.md › Ecualizador](HALLAZGOS.md#ecualizador), [Micrófono](HALLAZGOS.md#micrófono)).

1. ~~Niveles de THX por ZeroMQ~~: hecho el 2026-09-26 (cliente propio en `src/thx/`, [ADR 0005](adr/0005-thx-por-zeromq.md), sliders en MEJORAS). Queda confirmar de oído la normalización ("Esperando al usuario" 1).
2. ~~EQ como Synapse~~: hecho el 2026-09-26 ([ADR 0006](adr/0006-eq-como-synapse.md)), también con el botón EQ del headset y el panel abierto. Falta que el **proceso en segundo plano** siga al botón con el panel cerrado (hoy solo lo hace el panel).
3. ~~Sidetone como Synapse~~: no hizo falta; el sidetone del headset se oye con Synapse cerrado (2026-09-26). No se tocó el sidetone de THX. Si vuelve a fallar, el plan era fijar y activar el de THX (`IVSSrvSettings::SetInputSidetoneLevel`/`SetInputSidetoneState`, o abrir una captura del micrófono; ver [HALLAZGOS.md](HALLAZGOS.md#micrófono)).
4. ~~Micrófono por THX~~: hecho el 2026-09-26 ([ADR 0007](adr/0007-microfono-en-el-perfil.md)). Queda la prueba tras reiniciar ("Esperando al usuario" 1).
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
- Mejoras de micrófono con Equalizer APO o RNNoise, para quien no tenga THX.
- Importar de `.synapse4` también las mejoras del micrófono (`micVolumeNormalization`, `micVoiceClarity`, `ambientNoiseReduction`, `micSensitivity`, `micEqualizer`).
- Aviso de batería baja.

## Deuda técnica

- Solo hay pruebas unitarias; el camino página ↔ Rust no tiene prueba automática (se probó a mano con Xvfb). Una prueba con Playwright sobre `ui/` + `demo.js` cubriría la página.
- `demo.js` duplica a mano la forma del estado de `App::view`; si divergen, la demo miente. Se podría generar un estado de ejemplo desde Rust.
- El dispositivo predeterminado se cambia con PowerShell (interfaz no documentada de Windows).
- La interfaz COM de THX se declara a mano según la biblioteca de tipos de `VSSrv` 3.2.3.0; si THX la cambia, hay que revisarla ([ADR 0004](adr/0004-thx-por-com.md)).
- El cliente ZeroMQ y el protobuf de THX son propios y mínimos; si THX cambia sus mensajes, hay que capturar de nuevo ([ADR 0005](adr/0005-thx-por-zeromq.md)).

## Preguntas abiertas

- ¿Por qué el sidetone no se oyó en la primera prueba del 2026-09-26 y sí en la segunda? La diferencia más clara: en la segunda Synapse estaba cerrado. Si vuelve a fallar, anotar si Synapse estaba abierto.
- ¿El headset necesita que el modo remoto quede encendido para que el EQ se oiga? ¿O su curva Personalizada solo se oye sin THX instalado?
- ¿Qué hace realmente `0x9E` (Speaker Preset EQ Status)? En la ronda 1 no cambió nada.
- ¿THX sigue sonando si se detiene el servicio de THX (`VSSrv`)? Detenerlo requiere administrador y permiso del usuario.
- ¿Qué son los parámetros 1–3 de `GetParam` (COM)?
- ¿La normalización de THX se oye? El servicio la guarda igual que desde Synapse, pero con música no se notó.
- La curva de THX se suma a la del headset (y Synapse la cambia al pulsar el botón EQ). ¿Influyó en la ronda 1 de la prueba guiada? Conviene anotar el preset de THX activo al repetirla.

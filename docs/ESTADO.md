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
| Panel nuevo (WebView2) | 🟡 ⏳ | Probado en Linux (WebKitGTK) con `--demo` y compilado en el CI de Windows; **falta abrirlo en Windows**. |
| Prueba guiada del EQ (ronda 2) | 🟡 ⏳ | |
| Registro de depuración (`debug.log`) | 🟡 | |

### Investigación

- Qué hace Synapse con cada opción (captura): ✅ ver [HALLAZGOS.md](HALLAZGOS.md).
- THX es un efecto de audio de Windows, no del headset: ✅. Dónde guarda sus ajustes: ⏳ falta la captura `-Fase thx`.
- Claves de registro de las mejoras de Windows (Bass Boost, Loudness): ✅ capturadas; no implementadas.

## Esperando al usuario (en la PC)

1. **Abrir el panel nuevo en Windows** (artefacto `rzr-windows` del último build). Revisar: que abra sin parpadeo blanco, arrastrar un `.synapse4`, sliders y EQ.
2. **Ronda 2 de la prueba guiada** (AJUSTES › DIAGNÓSTICO) y enviar `debug.log`. Cerrar Synapse antes (se pelean por el dongle).
3. **Captura de THX:** `tools/capturar-synapse.ps1 -Fase thx` con Synapse instalado y THX sonando; enviar el `.zip`.

## Sigue (en orden)

1. **Hacer que la curva Personalizada se oiga.** Depende del resultado de la ronda 2. Si funciona "cambiar de preset y volver", dejarlo como método por defecto; si no, analizar `debug.log` contra la secuencia de OpenRazer ([HALLAZGOS.md](HALLAZGOS.md#ecualizador)).
2. **Confirmar el panel nuevo en Windows** y corregir lo que aparezca.
3. **Controlar THX desde rzr** con lo que muestre la captura (ver [ADR 0003](adr/0003-thx-por-sus-ajustes.md)): primero leer y mostrar el estado; luego activar y desactivar Bass Boost, Normalización, Claridad de voz y Spatial Audio.
4. **Revisar la configuración de audio de Windows:** avisar si las mejoras de audio están desactivadas o Windows Sonic encendido, y ofrecer corregirlo.
5. **Mejoras de Windows sin Synapse** (Bass Boost, Loudness) con las claves ya capturadas, para quien no tenga THX.
6. **Confirmar en hardware** lo marcado 🟡 y actualizar esta tabla.
7. **Publicar la versión 0.2.0** (release con `rzr.exe`) cuando 1 y 2 estén listos.

## Ideas (sin prioridad)

- Ícono en la bandeja del sistema para el proceso en segundo plano (batería, preset, abrir el panel).
- Otros modelos (BlackShark V3, HyperSpeed): OpenRazer los describe con una ficha por modelo; se podría hacer igual.
- Mejoras de micrófono con Equalizer APO o RNNoise.
- Aviso de batería baja.

## Deuda técnica

- Solo hay pruebas unitarias; el camino página ↔ Rust no tiene prueba automática (se probó a mano con Xvfb). Una prueba con Playwright sobre `ui/` + `demo.js` cubriría la página.
- `demo.js` duplica a mano la forma del estado de `App::view`; si divergen, la demo miente. Se podría generar un estado de ejemplo desde Rust.
- El dispositivo predeterminado se cambia con PowerShell (interfaz no documentada de Windows).

## Preguntas abiertas

- ¿El headset necesita que el modo remoto quede encendido para que el EQ se oiga?
- ¿Qué hace realmente `0x9E` (Speaker Preset EQ Status)? En la ronda 1 no cambió nada.
- ¿THX sigue sonando con Synapse cerrado y sus servicios detenidos? (la captura `-Fase thx` lo pregunta)
- ¿Dónde guarda THX sus ajustes: registro del dispositivo (FxProperties), archivos o comunicación directa con el efecto?

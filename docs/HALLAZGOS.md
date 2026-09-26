# Hallazgos

Lo que se ha averiguado sobre el headset, Synapse, THX y Windows: de dónde salió y qué tan seguro es. El protocolo USB (formato de frames y tabla de comandos) está en el [README](../README.md#cómo-funciona); aquí va el porqué y lo que aún no está en el código.

Cada hallazgo indica su **fuente**: captura de Synapse, OpenRazer, prueba en el headset del usuario o documentación.

## Headset y protocolo

- **El enlace se duerme** tras ~0,3 s sin tráfico y descarta el primer frame. Toda secuencia empieza con un frame de modo remoto "de sacrificio". _Fuente: OpenRazer (verificado en hardware)._
- **Cambio de familia:** un selector que cruza de Estándar a Esports (o al revés) solo cambia la familia; el headset cae en el preset que recordaba de esa familia. Hay que leer y reintentar. _Fuente: OpenRazer._
- **La curva se guarda en la ranura del preset activo**, sea cual sea. Por eso rzr confirma el preset antes de escribir. _Fuente: OpenRazer._
- **Juego, Película y Música editados en Synapse** solo cambian el EQ por software de THX; el headset sigue con su curva de fábrica. _Fuente: captura de Synapse; confirmado con `SetRenderEQGains` en la captura del 2026-09-25 (ver [THX](#thx-spatial-audio))._
- **Selectores Esports:** FA Apex, FB CS2, FC Valorant, FD Fortnite, FE CoD. Synapse escribe la curva de cada preset Esports en su ranura. _Fuente: captura de Synapse._
- **Sidetone:** Synapse convierte su escala 0–100 al headset en forma lineal (50 → 7, 31 → 4). _Fuente: captura de Synapse._ En este headset, con THX instalado, **no se oye**: el headset confirma `0x98 = 1` y `0x99 = 14` y `0x19` devuelve 14, pero el usuario no escucha su voz. Con THX instalado, Synapse además llama a `SetCaptureSidetoneLevel` de THX ([Micrófono](#micrófono)). _Fuente: `debug.log` y prueba del usuario, 2026-09-26._
- Una respuesta puede traer varios mensajes (una confirmación y un evento). _Fuente: captura de Synapse._
- **Formato de cada mensaje:** `PI`, tipo, secuencia, 4 bytes, **tamaño del cuerpo** (2 bytes, little-endian) y el cuerpo: comando, flag y, en las respuestas y eventos del headset (tipo `08`), largo y datos. Hay que avanzar por el tamaño del cuerpo: algunos cuerpos no traen largo (el aviso `E3` del dongle mide 2 bytes) y en los de tipo `01` y `11` el tercer byte no es un largo. Así se recorren sin error los 11 123 paquetes de un `debug.log`. _Fuente: `debug.log` del 2026-09-26._
- **Aviso del dongle `E3`** (tipo `0E`): `E3 00` al perderse el enlace y `E3 01` al volver, justo antes del evento `20` (estado del enlace) correspondiente. _Fuente: caída real del 2026-09-26 en `debug.log`._

## Ecualizador

Resultados en el headset del usuario (ronda 1 de la prueba guiada):

- La curva escrita en Personalizado **se confirma y se lee igual**, pero **no se oye** con ninguna de las tres secuencias probadas. El 2026-09-26, moviendo la curva desde el panel, tampoco se oyó.
- **El EQ que sí se oye es el de THX** (`SetCurrentModeEQGains`, ver [THX](#cómo-le-llegan-los-ajustes-a-thx)). Es lo que usa Synapse al editar los presets.
- **Qué hace Synapse con el EQ** (sondeo del 2026-09-26, con THX instalado; todo se oyó):
  - **Juego / Película / Música:** al headset el selector (`0x93` = 7 / 9 / 8) y la familia (`0x9D` = 1); a THX `SetPreset` (`Game Mode`, `Cinema Mode`, `Music Mode`) y la curva del preset de THX.
  - **Personalizado:** al headset el selector 255, la familia y la curva (`0x95`) en cada cambio; a THX `SetPreset` `Custom` y **la misma curva** (−5 a +5 dB). Restablecer manda todo a 0 por los dos lados.
  - **Al abrir**, Synapse vuelve a escribir las curvas de los cinco presets Esports y la Personalizada en sus ranuras, y deja el preset activo.
  - Con THX Spatial activado, THX guarda otra curva por preset: al activarlo, `eqCurve` pasó a plana y al desactivarlo volvió la anterior (las copias `-1-`/`-0-` de `UserState`). **Synapse no vuelve a mandar la curva** al cambiar Spatial (logs del 2026-09-25), así que su EQ cambia al activarlo. rzr sí la vuelve a mandar ([ADR 0006](adr/0006-eq-como-synapse.md)).
  - **Esports** (logs de Synapse del 2026-09-25): preset `Custom` de THX y la curva del preset Esports, igual que Personalizado. Synapse solo manda `SetRenderPreset` cuando cambia el preset de THX; la curva la manda siempre.
  - **Hecho en rzr** (2026-09-26, oído): preset de THX por ZeroMQ (`SetPreset`; la respuesta trae el estado con el preset nuevo) y curva por COM. Todo se oyó: los presets Estándar, los Esports y la curva Personalizada al arrastrarla.
- Cambiar `0x9E` no cambia nada.
- Cambiar entre Juego y Película **sí se oye**.

Comparación con OpenRazer (driver `razerblackshark`, [openrazer/openrazer#2862](https://github.com/openrazer/openrazer/pull/2862)):

- Sus secuencias son las mismas que usa rzr: modo remoto ×2 → consulta `0x1E` → selector `0x93` → familia `0x9D` → curva `0x95` → modo remoto.
- Para que una curva se oiga, OpenRazer escribe la curva y **después reenvía el selector** tras 100 ms (en una sola secuencia, el selector "carga" la curva anterior). rzr ya lo hace.
- Sus testers sí oyeron el EQ. La diferencia con este headset aún no se explica. La ronda 2 prueba: cambiar de preset y volver; presets Esports; curva en una ranura Esports; bajar todas las bandas −9 dB (se oye como bajar el volumen).
- OpenRazer separa lo que cambia entre modelos (V2 Pro, V3, HyperSpeed) en una ficha por modelo; por ejemplo, un desfase al escribir la curva. Útil si rzr soporta más modelos.

## THX Spatial Audio

- **No es parte del headset.** Synapse no envía ningún comando USB para Bass Boost, Normalización, Claridad de voz ni THX; los aplica con `AudioEffectsTHXV3.setRender…`. _Fuente: captura de Synapse._
- **Es un efecto de audio de Windows (APO)** que Synapse instala en la salida de los audífonos ("THX Spatial Audio (BlackShark V2 Pro)"), en lugar de los efectos de Windows. Corre dentro de `audiodg.exe` (`THXOutAPO-SSE2-v3.dll`, `THXMicAPO-SSE2-v3.dll`). _Fuente: ticket de soporte de Razer y captura._
- **Requisitos para que suene** (observado por el usuario tras reinstalar Synapse): las **mejoras de audio** de los audífonos activadas y **Windows Sonic** desactivado (Windows lo activó solo).
- **En Windows 11 los efectos se instalan como un paquete de driver.** Para respaldarlo (solo uso personal, nunca en el repo):

  ```
  pnputil /enum-drivers                      # buscar el oemNN.inf de Razer o THX
  pnputil /export-driver oemNN.inf C:\thx    # copia el paquete
  pnputil /add-driver C:\thx\*.inf /install  # reinstalar sin Synapse
  ```

- **Los ajustes de THX sí persisten sin Synapse.** Con Bass Boost al máximo, se siguió oyendo al cerrar Synapse y detener los servicios de Razer. _Fuente: captura `-Fase thx` (pasos 12 y 13), confirmado de oído por el usuario._ El **servicio de THX** (`VSSrv`) no es de Razer y sigue corriendo sin Synapse (verificado el 2026-09-26). Falta saber si el efecto sigue sonando con `VSSrv` detenido.

### Dónde guarda THX sus ajustes

_Fuente: captura `-Fase thx` y `-Fase synapse` del 2026-09-25 (fotos del registro en cada paso)._

| Lugar | Qué hay |
|---|---|
| `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{id}\Properties` (la salida de los audífonos) | `{d5e8f0ab-4de6-4d91-ab21-68868dda6a4a},6` (REG_SZ): **estado completo de THX en JSON**. `,7` (REG_BINARY): el mismo estado en protobuf. `,0` = `VSHP` (perfil de audífonos), `,1` = ruta a `C:\ProgramData\THX\usb\1532\0555\thx_spatial.conf.json`, `,11` y `,12` = 2. |
| `HKCU\Software\THX\SpatialAudio\UserState` | Una copia del estado (JSON con `snake_case`) por **preset de THX**, con nombre `<preset>-<0/1 espacial>-Headphones-usb\1532\0555`; además `CurrentPreset-usb\1532\0555-VSHP`, `EndpointScope-…` y `UserScope`. |
| `HKLM\SOFTWARE\THX\Discovery` | `thx:sa:service = tcp://127.0.0.1:49671` y `thx:sa:status:publisher = tcp://127.0.0.1:49670`: el **servicio de THX** escucha ahí. |
| Captura del micrófono, `{d5e8f0ab-…},0` | `VSMic`. Sus ajustes no cambiaron en el registro durante la captura (sin explicar aún). |

Ejemplo del JSON de `,6` (se reescribe entero en cada cambio):

```json
{"sequenceNumber":125,"spatialEnabled":false,"hardwareId":"usb\\1532\\0555","outputDevice":"Headphones",
 "presetName":"Music Mode","eqCurve":[0,2,2,2,2,2,2,1,1,1,1,1,1,2,2,2,3,3,3,3,3,3,3,3,3,1,1,1,0,0,0],"tilt":0,
 "spatialProcessingMode":"Headphones5","drcEnabled":false,"drcLevel":100,"emitterPositions":{…},"room":{…},
 "bassBoostEnabled":true,"bassBoost":100,"dialogEnhancementEnabled":false,"dialogEnhancement":100,"upmix":{…}}
```

| Opción de Synapse | Campo | Visto en la captura |
|---|---|---|
| THX Spatial Audio / Estéreo | `spatialEnabled` | `true` / `false` (también cambia el `-1-`/`-0-` de `CurrentPreset`) |
| Bass Boost | `bassBoostEnabled`, `bassBoost` | nivel 0–100; el nivel se conserva al apagarlo |
| Sound Normalization | `drcEnabled`, `drcLevel` | 0–100 |
| Voice Clarity | `dialogEnhancementEnabled`, `dialogEnhancement` | 0–100 |
| Preset Juego / Película / Música / Personalizado | `presetName` | `Game Mode`, `Cinema Mode`, `Music Mode`, `Custom`; cambia también `room` |
| Ecualizador | `eqCurve` | 31 valores: el primero es 0 y después cada una de las 10 bandas repetida 3 veces |
| — | `sequenceNumber` | sube en cada cambio |

- **Los presets de THX no son los del headset.** Juego/Película/Música/Personalizado en Synapse también eligen un preset de THX con su propia curva (EQ por software de 10 bandas; al arrastrar 1 kHz hasta arriba quedó en +5). Juego = `[-3,-3,-4,0,5,5,4,1,0,-1]`, Película = `[4,4,3,0,-3,-1,3,5,2,1]`, Música = `[2,2,1,1,2,3,3,3,1,0]`, Personalizado = plano. _Fuente: `ThxV3NativeSubProcess.log` (`SetRenderPreset` + `SetRenderEQGains`)._
- **El botón EQ del headset también cambia THX:** al pulsarlo, Synapse copió la curva de Juego al preset `Custom` de THX. Es decir, con Synapse abierto el sonido cambia por los dos lados (headset y THX). _Fuente: captura `-Fase synapse`, paso 33._ Puede influir en la prueba de la curva Personalizada.

### Cómo le llegan los ajustes a THX

_Fuente: sesión local en la PC del usuario, 2026-09-26: textos y biblioteca de tipos de `VSSrv.exe` y de `ThxV3Native`, llamadas de prueba y confirmación de oído por el usuario._

- Synapse usa `ThxV3Native` (versión 1.1.42.1), que trae `thxv3lib` 3.2.0.0 y **ZeroMQ 4.3.5**. Al iniciar "crea el cliente THX" y le envía llamadas como `SetRenderBassBoost`, `SetRenderBassBoostLevel`, `SetRenderNormalization`, `SetRenderVocalClarity`, `SetRenderSpatialProcessing`, `SetRenderPreset`, `SetRenderEQGains` (con `SetRenderBatch` 1/0 alrededor). _Fuente: `ThxV3NativeSubProcess.log`._
- **El servicio de THX** es `VSSrv` (`C:\Windows\System32\VSSrv.exe`, v3.2.3.0): arranque automático, corre como `LocalSystem`. Escucha en `127.0.0.1:49671` (un socket `ROUTER` de ZeroMQ: acepta REQ y DEALER) y `127.0.0.1:49670` (un `PUB`). Guarda el estado y lo escribe en el registro del dispositivo: el efecto no habla con nadie por red (`audiodg.exe` no tiene conexiones a esos puertos).
- **El servicio también es un servidor COM** con biblioteca de tipos (`VSSrvLib` 1.0, dentro de `VSSrv.exe`). La clase `VSSrv.CVSSrvTHXSettings` (`{2CCFC059-A1C5-4408-BCE5-0B23690DA7A2}`) implementa `IVSSrvTHXSettings` (`{3B3AF690-70A6-4DDA-BBEA-0B96493E9DA3}`), con `Init(procId)` y parejas `Get…`/`Set…`:

  | Método | Qué cambia |
  |---|---|
  | `SetSpatialProcessingState(originator, enabled, …)` | `spatialEnabled` ✅ oído (sutil con música) |
  | `SetBassBoostState(originator, enabled, …)` | `bassBoostEnabled` ✅ cambia el JSON; ❌ no se oyó con el nivel en 50 |
  | `SetDialogEnhanceState(originator, enabled, …)` | `dialogEnhancementEnabled` ✅ oído |
  | `SetDRCLevel(originator, nivel, …)` | solo `drcLevel`; **no** activa la normalización |
  | `SetCurrentModeEQGains(originator, float[31], …)` | `eqCurve` del preset de THX activo ✅ **oído al instante** (curva de "teléfono", −12 dB en graves y agudos) y restaurado. Mismo formato que `eqCurve`: un 0 y cada banda 3 veces. `GetCurrentModeEQGains` devuelve la curva. |
  | `SetProcessingMode`, `SetCustomRoomType`, `SetListeningMode`, `SetTiltEnabled`, `SetParam` | sin probar. `GetProcessingMode` = 3, `GetListeningMode` = 1, `GetTiltEnabled` = 0, `GetCustomRoomType` = 0 |

  - Un usuario normal puede crear el objeto y llamarlo (sin administrador). Los `Get…` devolvieron exactamente lo mismo que el JSON.
  - Tras un `Set…` el servicio reescribe `,6` y `,7` y sube `sequenceNumber`: entre 0,3 s y 3 s después. También lo publica en el `PUB` (tema `thx:sa:state`, con `x-originator:` y `x-payload:`).
  - **Probado con Synapse cerrado:** Spatial activado por COM se oyó claramente; desde el panel de rzr se oyó Claridad de voz y Spatial se notó poco con música. Cada cambio del panel quedó confirmado en el JSON en menos de 1 s.
  - COM no puede activar la **normalización** ni cambiar los **niveles** (Bass Boost, claridad de voz): eso va por ZeroMQ. `GetParam` responde solo a los parámetros 1–3 (valores 1, 1, 0) entre 0 y 300, sin saber qué son.
  - El búfer `inbandMessage` que devuelven los `Set…` vuelve vacío (`sz = 0`).
  - **Los cambios se aplican en vivo:** el efecto (`THXOutAPO`) se registra con el servicio (`IVSSrvTHXOutMFXAPO::NotifyAPOInit`), el servicio le avisa con un evento y el efecto pide el estado (`GetSystemState`). No hace falta reiniciar el audio.
- **Otras interfaces del servicio** (biblioteca de tipos de `VSSrv.exe`, sin probar):
  - `VSSrvSettings` (`{087E4DB6-0519-4A63-9099-9201915E1371}`), interfaz `IVSSrvSettings` (`{392375FF-8204-4A26-B4C5-AF4B71ACA729}`): `SetInputSidetoneState/Level`, `SetMicPreviewState`, `SetMicEQGains`, `SetMicParams`, `SetProcessingState`, `Set/GetOutputDeviceParams`. **Es el camino del micrófono** (ver [Micrófono](#micrófono)).
  - `IVSSrvTHXOutMFXAPO`, `IVSSrvInMFXAPO`, `IVSSrvOutMFXAPO`: las usan los efectos para leer su estado.
  - `IVSSrvAudioSession`, `IVSSrvRS3DSettings`: sesiones de audio y otro motor espacial.
- **ZeroMQ (lo que usa Synapse):** mensajes de varias partes, cada una con prefijo de texto: `x-originator:<nombre>` y `x-payload:<protobuf>` (y `x-Exception:` en errores). El protobuf es `thx.sa.THXMessage { Any msg = 1; string originator = 2; }`; el cliente se registra con `thx.sa.Register { uint32 pid = 1; }` y cambia ajustes enviando un `thx.sa.State` completo con `sequence_number` mayor que el actual.
  - `thx.sa.State` es el mismo mensaje guardado en `,7` (tras 8 bytes de cabecera `VT_BLOB`). Campos: 1 `sequence_number`, 2 `spatial_enabled`, 3 `hardware_id`, 4 `output_device`, 5 `preset_name`, 6 `eq_curve`, 7 `tilt`, 8 `spatial_processing_mode`, 9 `drc_enabled`, 10 `drc_level`, 11 `emitter_positions`, 12 `room`, 13 `bass_boost`, 14 `dialog_enhancement`, 15 `upmix`, 16 `Headphones5`, 17 `bass_boost_enabled`, 18 `dialog_enhancement_enabled`.
  - Enviando `Register` con ese formato (REQ y DEALER, con y sin `Init` previo por COM) el servicio **no respondió** y no cambió nada.
- **Conversación real de Synapse** (captura de loopback con `tshark`, 2026-09-26, Synapse reconectó durante la captura, así que incluye el saludo):
  - ZMTP 3.1, mecanismo `NULL`. El cliente es `REQ` con `Identity` vacía; el servicio responde como `REP`.
  - Cada petición tiene **cuatro partes**: una vacía (el delimitador de `REQ`), `x-address:tcp://127.0.0.1:<puerto>`, `x-originator:<uuid>` y `x-payload:<THXMessage>`. **El intento anterior no enviaba `x-address`.** El puerto de `x-address` es el anterior al de la conexión (`60018` para una conexión desde `60019`); probablemente otro socket del cliente.
  - La respuesta tiene dos partes: la vacía y `x-payload:` con `thx.sa.StateChangeResult { 2: mensaje ("State successfully changed"), 3: State }`.
  - Secuencia: `Register { pid }` → responde con el estado actual. Para elegir un preset: `thx.sa.SetPreset { 1: secuencia, 3: { 1: nombre, 4: "Headphones", 5: "usb\1532\0555" } }`, y después un `State` completo con la curva. Cada cambio de nivel (Bass Boost, Normalización, Claridad de voz) es un `State` completo con la secuencia siguiente.
  - Además, otra conexión pide la lista de presets con `thx.sa.PresetService.Request` y recibe `PresetService.Reply`.
  - En el `State`, `eq_curve` son 31 `double` (campo empaquetado).
- **Repetido desde un script, funciona** (2026-09-26, con Synapse cerrado): con `x-address` apuntando a un socket `PULL` propio, `Register` devuelve el estado. Un `State` igual al actual con la secuencia siguiente y `bass_boost` (campo 13, `double`) en 0 respondió "State successfully changed", el JSON quedó en `bassBoost: 0` y **se oyeron menos graves**; en 100 volvieron. ✅ oído. Es el camino para los niveles y la normalización.
- **Detalles comprobados después** (2026-09-26, sesión local):
  - `x-address` es obligatorio, pero el servicio **no se conecta** a esa dirección: responde igual aunque nadie escuche en ese puerto. rzr manda la dirección local de su propia conexión.
  - Acepta `x-originator:rzr`; no hace falta un UUID.
  - Las definiciones protobuf vienen dentro de `VSSrv.exe` (paquete `thx.sa`): `StateChangeResult { uint32 status = 1; string msg = 2; State state = 3 }` (`status` 0 = aceptado), `SetPreset { sequence_number = 1; PresetKey key = 3 }` y `PresetKey { name = 1; user = 2; spatial_enabled = 3; output_device = 4; hardware_id = 5 }`. Además de `thx.sa.PresetService`, hay `RoomService` y `EmitterService`.
  - **Desde rzr** (cliente propio, [ADR 0005](adr/0005-thx-por-zeromq.md)): cada cambio del panel (nivel de Bass Boost, normalización encendida o apagada y su nivel, claridad de voz) queda en el JSON en menos de 1 s. Los niveles de Bass Boost y Claridad de voz se oyen (0 ↔ 100).
  - **La normalización no se nota con música**, aunque el servicio la guarde: en una prueba a ciegas (normalización apagada, Spatial encendido, normalización encendida) el usuario solo notó el cambio de Spatial. Falta probar con audio de mucho contraste y compararlo con Synapse.
- **`spatial-config-util.exe` no sirve para los ajustes:** es un programa en Go que genera la configuración del dispositivo (`thx_spatial.conf.json`, filtros y presets en `C:\ProgramData\THX`). Sus opciones (`-usb`, `-hdaudio`, `-output-file`, `-writeout`, `-eq`, `-gameaux`…) no tocan Bass Boost ni los demás.
- **Registro:** la clave `Properties` del dispositivo hereda permiso de escritura para `Users` (`SetValue`), así que escribir `,6` no requiere administrador. No se probó: el servicio es quien lo mantiene, y escribirlo por fuera podría desincronizarlo.
- **Leer `,6` por `IPropertyStore` no sirve:** el almacén de propiedades del endpoint devuelve la cadena cortada en 259 caracteres (el JSON mide ~1 KB). rzr lo lee directamente del registro, lo que cualquier usuario puede hacer.
- **Caminos elegidos:** la interfaz COM del servicio para los interruptores que ofrece ([ADR 0004](adr/0004-thx-por-com.md)) y ZeroMQ para la normalización y los niveles ([ADR 0005](adr/0005-thx-por-zeromq.md)).

### Paquete de driver

_Fuente: `pnputil /enum-drivers` en la captura `-Fase thx`._ Todos firmados por Microsoft, versión 3.2.3.0 (24/06/2024): `thxrtapo.inf` (el efecto: `THXOutAPO-SSE2-v3.dll`, `THXMicAPO-SSE2-v3.dll`), `thxrtscu.inf` (`spatial-config-util.exe`), `thxrtsvc.inf` (servicio: `VSSrv.exe`, `VSHelper.exe`, `VSSrvInit.exe`) y `thxusbapo.inf` (lo asocia al USB `1532:0555`). Presets: "THX V3 APO Presets BlackSharkV2Pro2023 0555" 3.2.18.0.

_Fuente: sesión local del 2026-09-26 (`setupapi.dev.log`, "Aplicaciones instaladas", INF en `C:\Windows\INF`)._

- **`thxusbapo.inf` es el driver de la interfaz de audio del headset** (`USB\VID_1532&PID_0555&MI_00`, clase Media): usa el audio USB de Windows y agrega tres componentes de software: el efecto (`SWC\VEN_THX&CID_THXAPO`, `thxrtapo.inf`), el servicio (`VEN_THX&PID_THXSVC`, `thxrtsvc.inf`) y la configuración del modelo (`VEN_THX&PID_THXSCU_15320555`, `thxrtscu.inf`). Nada de esto depende de Synapse.
- **Synapse lo instala como programas aparte**, que aparecen en "Aplicaciones instaladas": "THX Spatial Audio USB 1532-0555" 3.2.3.0 (un paquete WiX con un MSI dentro) y "THX V3 APO Presets" 3.2.18.0. Por dentro corren `pnputil /add-driver … /install` desde `%TEMP%\THX`. Los instaladores quedan guardados en `C:\ProgramData\Package Cache`.
- **Desinstalar Synapse también desinstala THX** (observado por el usuario). Para quedarse con THX sin Synapse hay que respaldar antes los instaladores de `Package Cache` (se borran al desinstalar) y reinstalarlos después.

## Mejoras de audio de Windows

Capturadas con `-Fase windows` en `FxProperties\{b13412ee-07af-4c57-b08b-e327f8db085b}\User` del dispositivo de salida. Aún no se usan en rzr.

| Opción | Clave | Valores |
|---|---|---|
| Bass Boost | `{1864a4e0-efc1-45e6-a675-5786cbf3b9f0},4` (VT_UI4) | 2 activado / 0 apagado. La primera vez también escribe `{61e8acb9-…},4 = 80` y `{ae7f0b2a-…},3 = 1` |
| Loudness Equalization | `{fc52a749-4be9-4510-896e-966ba6525980},3` (VT_BOOL) | tiempo de liberación en `{9c00eeed-…},3 = 4` |
| Sonido envolvente virtual | — | cambia el formato de canales del dispositivo, no FxProperties |
| Deshabilitar todas las mejoras | `PKEY_AudioEndpoint_Disable_SysFx` `{1da5d803-d492-4edd-8c23-e0c0ffee7f0e},5` (DWORD) | 1 deshabilitadas / 0 habilitadas |

## Micrófono

- **Sin THX instalado**, las mejoras de micrófono de Synapse (EQ, normalización, claridad vocal, reducción de ruido, puerta de voz) no generaron comandos USB ni llamadas a THX. _Fuente: primera captura de Synapse._
- **Con THX instalado** sí van al motor THX: `SetCaptureNormalization(+Level)`, `SetCaptureVocalClarity(+Level)`, `SetCaptureNoiseReduction(+Level)`, `SetCaptureVoiceGate(+Level, en dB: −40 a −20)`, `SetCaptureEQGains` (10 bandas; 1 kHz al máximo = +12), `SetCapturePreview`, `SetCaptureSidetoneLevel` y `SetCaptureVolumeBySystem`. _Fuente: captura `-Fase synapse` del 2026-09-25 (`ThxV3NativeSubProcess.log`)._
- **Por dónde van** (sondeo del 2026-09-26): no pasan por ZeroMQ ni quedan en el registro. Van por COM a `IVSSrvSettings` del servicio de THX: después del sondeo, `GetMicEQGains` devolvió la curva del último preset elegido en Synapse y `GetInputSidetoneLevel` 0,3162 (Synapse mandó 3162: el nivel se divide por 10 000).
  - `GetMicParams` responde a 0–17 con parejas (encendido, nivel). Identificadas con un sondeo corto (2026-09-26: se leyó `IVSSrvSettings` cada 300 ms mientras el usuario ponía en Synapse niveles distintos a cada mejora):

    | Parámetros | Mejora | Nivel |
    |---|---|---|
    | 14 / 15 | Normalización | 0–100 |
    | 10 / 11 | Claridad de voz | 0–100 |
    | 6 / 7 | Reducción de ruido | 0–100 |
    | 2 / 3 | Puerta de voz | dB, de −40 a −20 |

    Sin identificar (no cambiaron): 0/1 = (0, 200), 4/5 = (0, 10), 8/9 = (0, 2), 12/13 = (0, 40), 16/17 = (1, 40). `GetMicPreviewState` tampoco cambió.
  - Cada cambio en Synapse aparece en `GetMicParams` en menos de 300 ms. Claridad de voz, al cambiar el nivel, se apaga, se ajusta el nivel y se vuelve a encender.
  - **EQ Personalizado del micrófono:** Synapse recuerda su propia curva; al volver a Personalizado la reenvía entera con `SetCaptureEQGains` y al headset solo `0x96 = 255`. `GetMicEQGains` la devuelve igual.
- **Presets de EQ del micrófono:** Synapse manda al headset `0x96` (Default 0, MicBoost 1, Broadcast 2, Conference 3, Personalizado 255) y a THX la curva (`SetCaptureEQGains`): MicBoost `[0,2,3,4,5,5,5,4,3,1]`, Broadcast `[4,4,4,3,-2,-7,-4,-2,-3,-5]`, Conference `[-8,-7,-5,-3,-1,1,3,2,1,0]`, Default plano. El Personalizado va de −12 a +12 y solo va a THX.
- **Sidetone con THX:** Synapse manda al headset `0x98`/`0x99` y además, a THX, `SetStartCaptureStatus(1)` y `SetCaptureSidetoneLevel(slider × 31,62)` (51 → 1613, 70 → 2214, 100 → 3162). Con Synapse **se oye**; solo con los comandos del headset (rzr), no. El sidetone de Synapse no pasa por las mejoras del micrófono (lo dice la propia interfaz de Synapse).
  - El nivel se ve en `IVSSrvSettings::GetInputSidetoneLevel` (70 → 0,2214). Pero `GetInputSidetoneState` **siguió en 0** con el sidetone encendido: lo que lo activa es `SetStartCaptureStatus`, que no se refleja en esa interfaz. Hipótesis sin probar: `ThxV3Native` abre una captura del micrófono para que el efecto del micrófono corra y mezcle la voz en la salida; rzr tendría que hacer lo mismo (abrir la captura por WASAPI mientras el sidetone esté encendido). Apagar el sidetone manda `SetStartCaptureStatus(0)` y `0x98 = 0`.
- **Volumen y silencio del micrófono:** `SetCaptureVolumeBySystem` (0–100) y `SetCaptureMuteBySystem`.
- Alternativas sin THX: Equalizer APO (EQ), NVIDIA Broadcast o RNNoise (ruido).

## Interfaz

- egui con OpenGL mostraba cuadros negros en Windows; con Direct3D 12 el exe pesaba 8,7 MB. Slint quedaba en ~8,5 MB. WebView2 deja el exe en ~1,3 MB. Ver [ADR 0001](adr/0001-interfaz-web-webview2.md).
- **WebView2 deja ~30 MB de caché** (285 archivos) en `%LOCALAPPDATA%\rzr\webview`, frente a un exe de 1,4 MB. El usuario prefiere una interfaz nativa y liviana. _Fuente: sesión local del 2026-09-26._

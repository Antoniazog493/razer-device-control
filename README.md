# rzr

Panel de control liviano para los audífonos **Razer BlackShark V2 Pro** (versión 2.4 GHz + Bluetooth, dongle `1532:0555`) que **reemplaza a Razer Synapse**. Es un solo `.exe` portátil de ~6 MB, sin instalador ni servicios en segundo plano.

![Pestaña Sonido](docs/sonido.png)

![Ecualizador](docs/ecualizador.png)

![Micrófono](docs/microfono.png)

## El problema

Al conectar los BlackShark V2 Pro sin Synapse, el audio suena bajo y plano: los presets integrados del headset son los de fábrica. Synapse envía la configuración real (preset del ecualizador, curva personalizada, etc.) por USB HID cada vez que cambias de perfil. Sin Synapse, esos comandos nunca llegan.

**rzr** envía exactamente los mismos comandos y además te da una interfaz como la de Synapse para configurar todo a tu gusto.

## Qué puedes controlar

| Función | Dónde vive | Estado |
|---|---|---|
| Ecualizador: presets Juego / Película / Música | Headset | ✅ |
| Ecualizador: presets Esports (Apex, CoD, CS2, Fortnite, Valorant) | Headset | ✅ |
| Ecualizador personalizado de 10 bandas (−5 a +5 dB) | Headset | ✅ |
| Monitoreo de micrófono (sidetone) y su nivel | Headset | ✅ |
| Apagado automático (15–60 min) | Headset | ✅ |
| No molestar (bloquear llamadas por Bluetooth) | Headset | ✅ |
| Batería, carga, estado del botón de silencio, firmware, serie | Headset (lectura) | ✅ |
| Volumen de salida y del micrófono, silenciar | Windows | ✅ |
| Dispositivo de salida/entrada predeterminado al conectar | Windows | ✅ |
| Varios perfiles, importar perfiles de Synapse (`.synapse4`) | rzr | ✅ |
| Iniciar con Windows y aplicar el perfil al conectar/reconectar | rzr | ✅ |
| Bass Boost, Normalización de sonido, Claridad de voz, THX Spatial Audio | Software de Synapse en el PC | ❌ ver abajo |
| EQ de micrófono, normalización, claridad vocal, reducción de ruido, puerta de voz | Software de Synapse en el PC | ❌ ver abajo |

### ¿Por qué faltan algunas funciones de Synapse?

Esas mejoras **no las hace el headset**. Synapse procesa el audio en tu PC con su propio driver/efecto de audio. El driver de OpenRazer, verificado con este mismo headset, confirma que el comando de "mejora" (`0x9D`) no tiene ningún efecto en el dispositivo. Por eso rzr no puede activarlas enviando comandos por USB. Para tenerlas sin Synapse habría que procesar el audio en el PC, por ejemplo con [Equalizer APO](https://sourceforge.net/projects/equalizerapo/) (bass boost, EQ de micrófono) o NVIDIA Broadcast / RNNoise (reducción de ruido).

## Descarga

- **Compilado automáticamente:** en la pestaña **Actions** del repositorio, abre la última ejecución de *Build* y descarga el artefacto `rzr-windows` (contiene `rzr.exe`).
- **Compilarlo tú:** instala [Rust](https://rustup.rs) (1.95 o superior) y ejecuta `cargo build --release`. El ejecutable queda en `target/release/rzr.exe`.

## Uso

Haz **doble clic en `rzr.exe`** y se abre el panel. Cada cambio se guarda y se envía al headset al instante. Los cambios de sliders y del ecualizador se envían al soltar.

- **Perfiles:** el menú `•••` junto a *PERFIL* permite crear, duplicar, renombrar, eliminar e importar perfiles.
- **Importar de Synapse:** exporta tu perfil desde Synapse (archivo `.synapse4`) y usa *Importar de Synapse…* o **arrástralo a la ventana**. Se importan el ecualizador, el sidetone, el apagado automático y No molestar.
- **Iniciar con Windows:** actívalo en *AJUSTES*. rzr quedará en segundo plano, sin ventana, y aplicará tu perfil cada vez que el headset se conecte o reconecte.
- **Botón EQ del headset:** si cambias de preset con el botón físico, el panel lo detecta y actualiza la selección.

### Línea de comandos

```
rzr                    Abre el panel
rzr apply              Aplica el perfil activo al headset
rzr import ARCHIVO     Importa perfiles de un .synapse4
rzr --watch            Vigila el headset y aplica el perfil al conectar
rzr --silent --watch   Lo mismo, en segundo plano sin salida (para el inicio)
rzr help               Ayuda
```

La configuración se guarda en `%APPDATA%\rzr\config.json`. La primera vez, rzr migra la configuración de versiones anteriores (`HKCU\SOFTWARE\rzr`).

## Cómo funciona

El dongle expone un endpoint HID propietario (interfaz USB 3, Usage Page `0xFF00`). Se usan reportes de 64 bytes con el protocolo "Audio MXIC" ("PA"):

```
[0]  0x02       report id
[1]  0x80       dirección (host → dispositivo)
[2]  total_len  8 + largo de datos
[5]  0x50 'P'   [6] 0x41 'A'
[7]  inner_len  0x08 (0x0E en el frame de modo remoto)
[9]  cmd_type   0x02 remoto, 0x03 lectura, 0x04 escritura, 0x0D EQ
[10] cmd_id
[11] flag       0 en una petición
[12] data_len
[13] datos...
```

Las respuestas repiten el sub-frame desplazado: `[12]` = id del comando, `[13]` = `0x01` (ACK), `[14]` = largo, `[15..]` = datos.

### Comandos

| Función | Escritura | Lectura | Datos |
|---|---|---|---|
| Modo remoto (antes de cada secuencia) | `02/E1` | — | 1 = software, 0 = headset (en el byte flag) |
| Selector de preset EQ | `04/93` | `03/13` | `07` Juego, `08` Música, `09` Película, `FF` Personalizado, `FA`–`FE` Esports |
| Familia del preset | `04/9D` | — | 1 = clásico, 2 = esports |
| Curva EQ personalizada | `0D/95` | `03/15` | 10 bytes con signo (dB) |
| Sidetone on/off | `04/98` | `03/18` | 0/1 |
| Nivel de sidetone | `04/99` | `03/19` | 1–10 |
| No molestar | `04/A7` | `03/27` | 0/1 |
| Apagado automático | `04/AC` | `03/2C` | minutos (15–60), 0 = nunca |
| Enlace inalámbrico | — | `03/20` | 1 = headset conectado |
| Batería / carga | — | `03/21` / `03/2A` | 0–100 / ≠0 = cargando |
| Botón de silencio | — | `03/55` | 1 = silenciado |
| Firmware / serie | — | `03/02` / `03/00` | |
| SET_CONFIG de Synapse | `06/01` | — | `C2 03 F8 5F 04` |

> **Corrección respecto a la versión anterior:** el comando `0x93` que antes se llamaba "setVolume" es en realidad el **selector de preset** (`255` = `0xFF` = Personalizado, por eso funcionaba), y `0x9D` ("setEnhancement") solo indica la familia del preset.

### Peculiaridades del firmware

- El enlace 2.4 GHz se duerme tras ~0,3 s sin tráfico y descarta el primer frame que recibe. Cada secuencia empieza con un frame de modo remoto "de sacrificio".
- Un cambio de preset que cruza de familia (clásico ↔ esports) solo cambia la familia. rzr lee el preset activo y reintenta hasta confirmarlo.
- El headset guarda una curva `0x95` en el preset que esté activo. rzr confirma que Personalizado está activo antes de escribirla. Luego reenvía el selector para que la curva nueva se escuche de inmediato.

## Código

| Archivo | Contenido |
|---|---|
| `src/protocol.rs` | Construcción de frames, tabla de comandos, presets |
| `src/device.rs` | Comunicación HID y secuencias de escritura verificadas |
| `src/config.rs` | Perfiles y configuración (JSON) |
| `src/synapse.rs` | Importador de `.synapse4` |
| `src/winaudio.rs` | Volumen, silencio y dispositivo predeterminado de Windows |
| `src/worker.rs` | Hilos en segundo plano (headset y audio) para la interfaz |
| `src/gui/` | Interfaz (egui) con estilo Synapse |
| `src/registry.rs` | Migración del registro e inicio con Windows |

`rzr --demo` abre el panel con un headset simulado, útil para probar la interfaz sin el dispositivo.

## Créditos

- Protocolo original por ingeniería inversa de Synapse 4: [Ashesh3/razer-device-control](https://github.com/Ashesh3/razer-device-control).
- Tabla de comandos y secuencias verificadas en hardware: driver de OpenRazer para este headset ([openrazer/openrazer#2862](https://github.com/openrazer/openrazer/pull/2862)).

## Licencia

MIT

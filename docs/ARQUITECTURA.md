# Arquitectura

Cómo está construido rzr. Si cambias la estructura (un módulo nuevo, otro hilo, otro archivo en disco), actualiza este documento en el mismo commit.

## Un ejecutable, tres modos

`rzr.exe` es un solo archivo portátil. Según cómo se inicie:

| Modo | Cómo se inicia | Qué hace |
|---|---|---|
| **Panel** | doble clic, `rzr`, `rzr --demo` | Abre la ventana. Mientras está abierta, aplica el perfil cuando el headset se conecta (si no lo hace ya el proceso en segundo plano). |
| **Segundo plano** | `rzr --silent --watch` (lo que usa "Iniciar con Windows") | Sin ventana. Vigila el dongle y aplica el perfil en cada conexión. Solo puede haber uno. |
| **Línea de comandos** | `rzr apply`, `rzr import ARCHIVO`, `rzr help` | Hace una cosa y termina. |

El panel y el proceso en segundo plano pueden correr a la vez: comparten el dongle mediante un candado (ver [Concurrencia](#hilos-y-concurrencia)).

## Capas

```
 ui/index.html + app.css + app.js       La página: dibuja y manda comandos
          │  ▲
 comandos │  │ estado (JSON)            window.ipc.postMessage / rzr.state(...)
          ▼  │
 gui/window.rs                          Ventana (tao) + WebView (wry: WebView2 / WebKitGTK)
 gui/mod.rs  (App)                      Controlador: config, comandos → cambios, estado → página
          │  ▲
 DevCmd   │  │ DevEvent                 canales (mpsc) + aviso Notify
          ▼  │
 worker.rs                              Hilo del headset (sondeo, eventos, escrituras)
                                        Hilo de audio de Windows (volumen, dispositivos)
                                        Hilo de THX (estado y cambios por su servicio)
          │
 device.rs                              Secuencias: aplicar perfil, elegir preset, escribir curva
 protocol.rs                            Formato de los frames HID y tabla de comandos
          │
 hidapi → dongle (USB, interfaz 3, 64 bytes) → headset
```

Cada capa solo conoce a la de abajo. `protocol.rs` no sabe de hilos; `device.rs` no sabe de la interfaz; la página no sabe del protocolo.

## Módulos

| Archivo | Responsabilidad |
|---|---|
| `src/main.rs` | Lee los argumentos y elige el modo. Contiene los bucles de `--watch` y `apply`. |
| `src/protocol.rs` | Construye y analiza frames; constantes de comandos; presets, selectores y curvas de referencia. Sin E/S. |
| `src/device.rs` | Abre el dongle y ejecuta secuencias con reintentos y verificación por lectura. Toma el candado del bus. |
| `src/worker.rs` | Hilos del panel: el del headset (sondeo cada 5 s, eventos cada 250 ms, escrituras), el de audio de Windows y el de THX. Incluye el headset simulado de `--demo`. |
| `src/gui/mod.rs` | `App`, el controlador del panel: recibe `Msg` de la página, cambia la config, pide escrituras al hilo del headset y arma el estado (`App::view`). |
| `src/gui/window.rs` | Ventana, WebView, protocolo `rzr://`, bucle de eventos, guardado diferido. |
| `src/gui/assets.rs` | Archivos de `ui/` incluidos en el exe (o leídos de `RZR_UI_DIR`). |
| `src/gui/diag.rs` | Lógica de la prueba guiada (sin interfaz). |
| `src/config.rs` | Perfiles y ajustes; carga, valida (`sanitize`) y guarda `config.json`. |
| `src/synapse.rs` | Importa perfiles `.synapse4`. |
| `src/winaudio.rs` | Volumen, silencio, lista de dispositivos y dispositivo predeterminado de Windows. |
| `src/thx.rs` | Estado de THX (el JSON del registro de la salida de los audífonos) y sus cambios: los interruptores por la interfaz COM del servicio de THX ([ADR 0004](adr/0004-thx-por-com.md)), la normalización y los niveles por ZeroMQ ([ADR 0005](adr/0005-thx-por-zeromq.md)). |
| `src/thx/zmtp.rs` | Lo mínimo de ZeroMQ para hablar con el servicio de THX: saludo ZMTP 3.1 y tramas de un socket `REQ`. Sin bibliotecas. |
| `src/thx/proto.rs` | Los mensajes protobuf del servicio de THX (`Register`, `State`, su respuesta). Sin E/S. |
| `src/connlog.rs` | Registro de caídas del enlace (`conexion.log`). |
| `src/debuglog.rs` | Registro de depuración opcional (`debug.log`) y la macro `dlog!`. |
| `src/instance.rs` | Mutex de instancia única del proceso en segundo plano y candado del bus. |
| `src/registry.rs` | "Iniciar con Windows" y migración de la configuración antigua del registro. |
| `ui/` | La interfaz. `demo.js` solo se usa al abrirla en un navegador. |
| `tools/` | Scripts de investigación: `capturar-synapse.ps1` (capturas) y `decodificar-log.py`. |

## Flujo de un cambio

Ejemplo: el usuario elige el preset CS2.

1. `app.js` envía `{"cmd": "preset", "preset": "csgo"}` con `window.ipc.postMessage`.
2. `window.rs` recibe el texto, lo convierte en `Msg::Preset` y llama a `App::handle`.
3. `App` cambia el perfil, programa el guardado (500 ms después) y envía `DevCmd::Update(Target, Change::Eq)` al hilo del headset.
4. El hilo del headset llama a `Device::set_eq`, que toma el candado, envía la secuencia y lee el preset para confirmarlo.
5. El hilo avisa (`DevEvent` + `Notify`). `window.rs` despierta, `App::pump` recoge el aviso y, como el estado cambió, la página recibe `rzr.state({...})`.
6. `app.js` redibuja con el estado nuevo. La página nunca asume que un cambio funcionó: muestra lo que dice el estado.

## Contrato entre la página y Rust

- **Comandos (página → Rust):** el enum `Msg` en `src/gui/mod.rs`. JSON con un campo `cmd` en snake_case y sus argumentos. Un comando desconocido se ignora y queda en `debug.log`.
- **Estado (Rust → página):** `App::view()` en `src/gui/mod.rs`. Se envía completo cada vez que algo cambia; la página no guarda estado propio salvo lo visual (pestaña abierta, arrastre en curso, diálogos locales).
- **Avisos:** `rzr.toast(texto, error)`.
- **Enlaces en el HTML:** `data-text`, `data-show`, `data-level-from`, `data-toggle`, `data-slider`, `data-send`, `data-open` (descritos al inicio de `ui/index.html`). Con ellos, la mayoría de las funciones nuevas no necesitan JavaScript.

`ui/demo.js` imita a Rust con datos fijos. Si cambias el estado o los comandos, actualízalo también.

## Hilos y concurrencia

- **Hilo principal:** el bucle de eventos de la ventana (tao). Todo lo de `App` corre aquí; nunca se bloquea con E/S del headset.
- **Hilo del headset:** único dueño del `Device`. Recibe `DevCmd` y agrupa los que llegan juntos (varios cambios se aplican una vez).
- **Hilo de audio:** habla con Core Audio de Windows (COM). Cada 2 s envía el estado de volumen y dispositivos.
- **Hilo de THX:** cada 2 s lee el estado de THX. Al cambiar una opción espera (hasta 6 s) a que el servicio de THX la guarde; va aparte para no frenar el volumen mientras tanto.
- **Entre procesos:**
  - `Local\rzr_hid_bus`: candado alrededor de cada secuencia. El panel y el proceso en segundo plano tienen el dongle abierto a la vez; sin él, una consulta de uno puede cortar la escritura del otro.
  - `Global\rzr_blackshark_v2_pro`: garantiza un solo proceso en segundo plano. El panel lo consulta para no duplicar el registro de caídas.

## Archivos en disco

| Ruta | Contenido |
|---|---|
| `%APPDATA%\rzr\config.json` | Perfiles y ajustes. Se escribe a un `.tmp` y se renombra (no queda a medias). |
| `%APPDATA%\rzr\conexion.log` | Caídas del enlace (máx. 256 KB). |
| `%APPDATA%\rzr\debug.log` | Registro de depuración, si está activado (máx. 4 MB, el anterior en `debug.old.log`). |
| `%LOCALAPPDATA%\rzr\webview\` | Caché de WebView2. |
| `HKCU\...\CurrentVersion\Run` | Entrada de "Iniciar con Windows". |
| `HKCU\SOFTWARE\rzr` | Configuración de versiones antiguas; solo se lee para migrarla. |

En Linux (desarrollo) la configuración va en `~/.config/rzr/`.

## Compilación y CI

- `Cargo.toml`: WebView2 vía `wry` + `tao` en Windows; WebKitGTK en Linux. Perfil release optimizado por tamaño (`opt-level = "s"`, LTO).
- `.github/workflows/build.yml` (Windows): formato, clippy sin avisos, sintaxis de la página, pruebas y compilación. El `rzr.exe` queda como artefacto `rzr-windows`.

## Pruebas

Pruebas unitarias sin hardware (`cargo test`):

- **Protocolo:** frames idénticos a los de la primera versión (que funcionaban) y respuestas capturadas de Synapse.
- **Configuración:** validación de rangos y JSON incompleto.
- **Importador de Synapse** y **migración del registro**.
- **Registro de caídas.**
- **THX:** análisis del estado (con un JSON escrito a mano) y la clave del registro de cada salida; mensajes protobuf contra una respuesta real del servicio (`src/thx/testdata/`); tramas ZeroMQ contra un servicio simulado.
- **Comandos de la página** y **archivos servidos.**
- **Flujo de la prueba guiada.**

Lo que depende del headset real se prueba a mano y queda anotado en [ESTADO.md](ESTADO.md).

## Receta: agregar una función del headset

1. **Protocolo:** constante del comando y constructor del frame en `protocol.rs`, con una prueba que compare los bytes con una captura.
2. **Secuencia:** método en `device.rs` que tome el candado y, si se puede, lea el valor para confirmarlo.
3. **Perfil:** campo en `Profile` (`config.rs`), con valor por defecto y límites en `sanitize`.
4. **Aplicar:** agrégalo a `Device::apply_profile` y, si se cambia suelto, una variante de `Change` en `worker.rs`.
5. **Controlador:** variante de `Msg`, su rama en `App::handle` y el dato en `App::view`.
6. **Página:** el HTML con sus enlaces `data-*`, el comando en `demo.js` y, si el usuario lo nota, una línea en el CHANGELOG.
7. **Documentación:** ESTADO.md y, si aplica, la tabla de comandos del README.

# Hallazgos

Lo que se ha averiguado sobre el headset, Synapse, THX y Windows: de dónde salió y qué tan seguro es. El protocolo USB (formato de frames y tabla de comandos) está en el [README](../README.md#cómo-funciona); aquí va el porqué y lo que aún no está en el código.

Cada hallazgo indica su **fuente**: captura de Synapse, OpenRazer, prueba en el headset del usuario o documentación.

## Headset y protocolo

- **El enlace se duerme** tras ~0,3 s sin tráfico y descarta el primer frame. Toda secuencia empieza con un frame de modo remoto "de sacrificio". _Fuente: OpenRazer (verificado en hardware)._
- **Cambio de familia:** un selector que cruza de Estándar a Esports (o al revés) solo cambia la familia; el headset cae en el preset que recordaba de esa familia. Hay que leer y reintentar. _Fuente: OpenRazer._
- **La curva se guarda en la ranura del preset activo**, sea cual sea. Por eso rzr confirma el preset antes de escribir. _Fuente: OpenRazer._
- **Juego, Película y Música editados en Synapse** solo cambian el EQ por software de THX; el headset sigue con su curva de fábrica. _Fuente: captura de Synapse._
- **Selectores Esports:** FA Apex, FB CS2, FC Valorant, FD Fortnite, FE CoD. Synapse escribe la curva de cada preset Esports en su ranura. _Fuente: captura de Synapse._
- **Sidetone:** Synapse convierte su escala 0–100 al headset en forma lineal (50 → 7, 31 → 4). _Fuente: captura de Synapse._
- Una respuesta puede traer varios mensajes (una confirmación y un evento). _Fuente: captura de Synapse._

## Ecualizador

Resultados en el headset del usuario (ronda 1 de la prueba guiada):

- La curva escrita en Personalizado **se confirma y se lee igual**, pero **no se oye** con ninguna de las tres secuencias probadas.
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

- **Hipótesis principal:** THX lee sus ajustes del registro del dispositivo (FxProperties) o recibe avisos de cambio de Windows. Si es así, rzr solo tiene que escribir esos valores. Otras posibilidades: archivos propios o comunicación directa con el efecto. La captura `-Fase thx` y, si hace falta, Process Monitor (filtrar procesos de Razer y `audiodg.exe`, operaciones `RegSetValue` y `WriteFile`) lo resuelven.

## Mejoras de audio de Windows

Capturadas con `-Fase windows` en `FxProperties\{b13412ee-07af-4c57-b08b-e327f8db085b}\User` del dispositivo de salida. Aún no se usan en rzr.

| Opción | Clave | Valores |
|---|---|---|
| Bass Boost | `{1864a4e0-efc1-45e6-a675-5786cbf3b9f0},4` (VT_UI4) | 2 activado / 0 apagado. La primera vez también escribe `{61e8acb9-…},4 = 80` y `{ae7f0b2a-…},3 = 1` |
| Loudness Equalization | `{fc52a749-4be9-4510-896e-966ba6525980},3` (VT_BOOL) | tiempo de liberación en `{9c00eeed-…},3 = 4` |
| Sonido envolvente virtual | — | cambia el formato de canales del dispositivo, no FxProperties |
| Deshabilitar todas las mejoras | `PKEY_AudioEndpoint_Disable_SysFx` `{1da5d803-d492-4edd-8c23-e0c0ffee7f0e},5` (DWORD) | 1 deshabilitadas / 0 habilitadas |

## Micrófono

Las mejoras de micrófono de Synapse (EQ, normalización, claridad vocal, reducción de ruido, puerta de voz) no generan comandos USB ni llamadas a THX. _Fuente: captura de Synapse._ Alternativas sin Synapse: Equalizer APO (EQ), NVIDIA Broadcast o RNNoise (ruido).

## Interfaz

- egui con OpenGL mostraba cuadros negros en Windows; con Direct3D 12 el exe pesaba 8,7 MB. Slint quedaba en ~8,5 MB. WebView2 deja el exe en ~1,3 MB. Ver [ADR 0001](adr/0001-interfaz-web-webview2.md).

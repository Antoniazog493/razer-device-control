# 0005. Lo que COM no puede cambiar en THX va por ZeroMQ, con un cliente propio

- Estado: Aceptada
- Fecha: 2026-09-26
- Amplía a [0004](0004-thx-por-com.md): COM sigue para lo que ya hacía.

## Contexto

[ADR 0004](0004-thx-por-com.md) eligió la interfaz COM del servicio de THX (`VSSrv`), pero esa interfaz no puede activar la normalización ni cambiar los niveles (Bass Boost, Normalización, Claridad de voz). Sin el nivel, Bass Boost al 50 no se oía.

Synapse cambia todo eso por la otra entrada del servicio: ZeroMQ en `127.0.0.1:49671`. En 0004 esa vía se descartó porque el servicio no respondía. Una captura de la conversación real de Synapse (2026-09-26) mostró qué faltaba: cada petición lleva cuatro partes, incluida `x-address:`. Con eso, un script cambió el nivel de Bass Boost y se oyó. Detalle en [HALLAZGOS.md](../HALLAZGOS.md#cómo-le-llegan-los-ajustes-a-thx).

rzr solo necesita una fracción de ZeroMQ: un socket `REQ`, un solo servicio en la misma PC, sin seguridad (`NULL`) y dos mensajes (`Register` y `State`, más `SetPreset` para los presets de THX).

## Decisión

- **Reparto de caminos:**
  - Por COM, como hasta ahora (ya verificado de oído): los interruptores de Spatial, Bass Boost y Claridad de voz.
  - Por ZeroMQ: la normalización y los tres niveles. Más adelante, también el preset de THX.
- **Cliente propio, sin bibliotecas:**
  - `src/thx/zmtp.rs`: saludo ZMTP 3.1 con `NULL`, comando `READY` y tramas de un socket `REQ`.
  - `src/thx/proto.rs`: el protobuf mínimo de esos mensajes.
- **Cómo se cambia un ajuste:**
  1. Se abre una conexión por cambio y se envía `Register` (el servicio responde con su estado).
  2. Se devuelve **ese mismo estado** con la secuencia siguiente y el campo cambiado. Los campos que rzr no entiende (sala, emisores, curva…) viajan byte por byte.
  3. Se confirma el cambio en la respuesta del servicio y después en el JSON del registro, igual que con COM.
- **Dirección del servicio:** se lee de su clave de descubrimiento (`HKLM\SOFTWARE\THX\Discovery`, `thx:sa:service`). Solo se aceptan direcciones de la propia PC. Si la clave no se puede leer, se usa `127.0.0.1:49671`.
- **`x-address`:** lleva la dirección local de la conexión. El servicio exige esa parte, pero no se conecta a ella (comprobado: responde aunque nadie escuche en ese puerto).
- **Originador:** `rzr`, como en COM (el servicio lo acepta).

## Alternativas

- **Todo por ZeroMQ, como Synapse:** un solo camino. Pero habría que volver a verificar de oído lo que ya funciona por COM, y COM seguirá haciendo falta para el micrófono y el sidetone (`IVSSrvSettings`). El usuario prefirió conservar COM donde ya funciona.
- **Crate `zeromq` (Rust puro):** es asíncrona, así que arrastra `tokio` o `async-std`. Demasiado peso para dos mensajes.
- **Crate `zmq` (enlaza libzmq en C):** exige compilar C (cmake) y complica compilar para Windows desde Linux.
- **Crate `prost` para el protobuf:** necesita los `.proto` y un paso de compilación. Aquí se leen o cambian seis campos, y el resto se copia sin interpretarlo.
- **Escribir el registro (`,6`/`,7`):** se descartó en 0004 y sigue descartado.

## Consecuencias

- **Sin dependencias nuevas:** el exe pasa de 1 467 392 a 1 536 512 bytes (+68 KB).
- **Protocolo sin documentar:** si THX cambia los mensajes o los números de campo, hay que capturarlo de nuevo. Para cuidarlo:
  - las pruebas usan una respuesta real del servicio (`src/thx/testdata/register-reply.bin`);
  - cada cambio se confirma en la respuesta y en el registro, y si no se confirma se avisa.
- **Estado completo en cada cambio:** entre el `Register` y el `State` pasan milisegundos, pero si otro cliente (Synapse abierto) cambia algo justo entonces, se pisaría. El servicio rechaza secuencias viejas, así que no queda un estado a medias.
- **Solo lo necesario de ZMTP:** una petición a la vez, sin reconexión automática, sin seguridad (el servicio usa `NULL`), y se ignoran los comandos entre mensajes (latidos). Si el servicio pidiera otro mecanismo, la conexión falla con un mensaje claro.

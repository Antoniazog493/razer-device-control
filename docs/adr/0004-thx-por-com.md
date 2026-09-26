# 0004. Los ajustes de THX se cambian por la interfaz COM de su servicio

- Estado: Aceptada. Ampliada por [0005](0005-thx-por-zeromq.md): normalización y niveles por ZeroMQ
- Fecha: 2026-09-26
- Completa a [0003](0003-thx-por-sus-ajustes.md): elige cómo escribir los ajustes.

## Contexto

[ADR 0003](0003-thx-por-sus-ajustes.md) decidió controlar THX escribiendo sus ajustes, sin redistribuir nada de Razer ni de THX, y dejó abierto el camino. Una sesión local en la PC del usuario encontró que el servicio de THX (`VSSrv.exe`, del paquete de driver de THX, no de Razer) ofrece dos entradas:

- **ZeroMQ**, en `127.0.0.1:49671`: es lo que usa Synapse.
- **Un servidor COM** con biblioteca de tipos (`VSSrv.CVSSrvTHXSettings`, interfaz `IVSSrvTHXSettings`).

Detalle en [HALLAZGOS.md](../HALLAZGOS.md#cómo-le-llegan-los-ajustes-a-thx).

## Decisión

- rzr cambia los ajustes de THX con `IVSSrvTHXSettings`: `Init(pid)` y los `Set…State` de Spatial, Bass Boost y Claridad de voz.
- Cada cambio se confirma releyendo el estado de THX (el JSON `,6` del registro de la salida de los audífonos) hasta que `sequenceNumber` sube y el campo coincide.
- rzr lee ese estado directamente del registro, sin escribirlo nunca.

## Alternativas

- **ZeroMQ, como Synapse:** permitiría todo (también activar la normalización y los niveles, enviando un `thx.sa.State` completo). Pero el servicio no respondió a los mensajes armados según lo averiguado, y hablarlo exige imitar un protocolo sin documentar: protobuf, prefijos `x-…` y registro previo. Queda pendiente de una captura real.
- **Escribir el registro (`,6`/`,7`):** no requiere administrador, pero salta al servicio, que es quien guarda el estado y avisa a sus clientes. Podría desincronizarlos o ser sobrescrito.
- **`spatial-config-util.exe`:** solo genera la configuración del dispositivo; no cambia ajustes.

## Consecuencias

- **Funciona sin Synapse y sin administrador:** probado en el headset del usuario con Synapse cerrado; Spatial y Claridad de voz se oyeron.
- **Depende de la biblioteca de tipos de `VSSrv` 3.2.3.0.** Si THX cambia el orden de los métodos, las llamadas irían a otro método. Para cuidarlo:
  - la interfaz se declara completa hasta el último método usado, en el orden de la biblioteca de tipos;
  - cada cambio se confirma releyendo el estado, y si no se confirma se avisa.
- **Límite de COM:** no puede activar la normalización ni cambiar niveles. La página los muestra, pero solo se cambian desde Synapse hasta que se entienda el camino ZeroMQ.
- **Sin THX o sin su servicio**, MEJORAS lo dice y no falla.

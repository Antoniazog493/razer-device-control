# 0006. El preset del headset elige también el preset y la curva de THX

- Estado: Aceptada
- Fecha: 2026-09-26

## Contexto

La curva Personalizada se guarda en el headset y se lee igual, pero **no se oye** (ver [ESTADO.md](../ESTADO.md)). El EQ que sí se oye con Synapse es el de THX: un EQ por software de 10 bandas con sus propios presets (`Game Mode`, `Cinema Mode`, `Music Mode`, `Custom`).

Los logs de Synapse (`ThxV3NativeSubProcess.log`, 2026-09-25) muestran qué hace al elegir un preset en el headset:

- **Juego / Película / Música:** elige el preset de THX correspondiente (`SetRenderPreset`) y le manda su curva (`SetRenderEQGains`), que es la misma que la de fábrica del headset.
- **Personalizado y los cinco Esports:** elige el preset `Custom` de THX y le manda la curva del preset.
- **No** vuelve a mandar la curva al activar o desactivar Spatial, aunque THX guarda otra curva por preset con Spatial encendido. Al cambiar Spatial, el EQ pasa a esa otra curva (a menudo plana).

Detalle en [HALLAZGOS.md](../HALLAZGOS.md#ecualizador).

## Decisión

- Cada vez que cambia el EQ del perfil, rzr manda al headset lo mismo que antes y, si THX está instalado, también el preset y la curva de THX, como Synapse. Pasa al elegir un preset, mover la curva, restablecerla, cambiar de perfil, aplicarlo o pulsar el botón EQ del headset.
- **Caminos:**
  - el preset de THX, por ZeroMQ (`thx.sa.SetPreset`, [ADR 0005](0005-thx-por-zeromq.md)), solo si cambia;
  - la curva, por COM (`SetCurrentModeEQGains`, ya verificado de oído), releyéndola con `GetCurrentModeEQGains`.
- **Confirmación:** el cambio se da por hecho cuando el JSON del registro muestra el preset y la curva. Si ya los muestra, no se envía nada.
- **Curvas seguidas:** si llegan varias juntas (al arrastrar la curva), solo se aplica la última.
- **Después de cambiar Spatial**, rzr vuelve a aplicar la última curva pedida. Es la única diferencia a propósito con Synapse: así la curva del perfil se oye con Spatial encendido o apagado.

## Alternativas

- **Solo el headset, como hasta ahora:** la curva no se oye en este headset. La causa sigue sin conocerse (ronda 2 de la prueba guiada).
- **Solo THX, sin escribir en el headset:** el headset seguiría con otro preset, y su botón EQ y lo que muestra el panel dejarían de cuadrar. Además, la curva del headset quizá se oiga sin THX instalado.
- **Curva por ZeroMQ (`State` con `eq_curve`), como Synapse:** posible, pero COM ya está verificado de oído y el usuario prefirió conservar COM donde funciona ([ADR 0005](0005-thx-por-zeromq.md)).
- **Imitar a Synapse también con Spatial** (no volver a mandar la curva): el EQ "desaparece" al activar Spatial, que parece un fallo.

## Consecuencias

- **El EQ se oye** (verificado en el headset real el 2026-09-26): los presets Estándar, los Esports, la curva Personalizada al arrastrarla y el cambio de Spatial.
- **Personalizado y los Esports comparten el preset `Custom` de THX:** al elegir uno, se pisa la curva del otro en THX. Da igual, porque rzr guarda las curvas en el perfil y las vuelve a mandar. También es lo que hace Synapse.
- **Sin THX o sin su servicio,** el EQ va solo al headset, como antes.
- **El proceso en segundo plano** (`--watch`) no toca THX: su estado queda en Windows y se mantiene entre reinicios. Solo el panel lo cambia.

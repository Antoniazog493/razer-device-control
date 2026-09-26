# 0007. Las mejoras del micrófono se guardan en el perfil y rzr se las vuelve a mandar a THX

- Estado: Aceptada
- Fecha: 2026-09-26

## Contexto

Las mejoras del micrófono (EQ con presets, normalización, claridad vocal, reducción de ruido y puerta de voz) no las hace el headset: las hace el efecto de micrófono de THX. Synapse las manda al servicio de THX (`VSSrv`), que las recibe por COM en `IVSSrvSettings` (`SetMicEQGains`, `SetMicParams`). Al headset solo le manda el índice del preset de EQ (`0x96`). Detalle en [HALLAZGOS.md](../HALLAZGOS.md#micrófono).

A diferencia de los ajustes de salida (el JSON en el registro de la salida de los audífonos), **no se encontró dónde guarda THX las del micrófono**. No aparecen en `HKCU\Software\THX`, ni en `HKLM\SOFTWARE\THX`, ni en el registro del micrófono. Lo más probable es que vivan solo en la memoria del servicio y que Synapse las vuelva a mandar cada vez que arranca. Sin Synapse, se perderían al reiniciar la PC. Además, Synapse las guarda **por perfil** (`micVolumeNormalization`, `micVoiceClarity`, `ambientNoiseReduction`, `micSensitivity`, `micEqualizer`).

## Decisión

- **Dónde se guardan:** en el perfil de rzr (`Profile::mic`), como en Synapse.
- **Cuándo se mandan a THX:**
  - al cambiarlas o al cambiar de perfil;
  - cada vez que rzr se conecta (o se reconecta) al servicio de THX: al abrir el panel y en el proceso en segundo plano, por ejemplo después de reiniciar Windows.
- **Qué se manda:** rzr lee lo que tiene el servicio y manda solo lo que difiere; después lo relee para confirmarlo. Como Synapse, apaga y vuelve a encender la claridad vocal al cambiar su nivel.
- **Al headset** se le manda `0x96` con el preset de EQ del micrófono (Predeterminado 0, Refuerzo 1, Transmisión 2, Conferencia 3, Personalizado 255), con los mismos bytes que Synapse.
- **Reconexión:** rzr detecta que el servicio se reinició con una llamada barata (`GetMicPreviewState`), se reconecta y vuelve a mandarlas.
- **No se reenvían en cada sondeo.** Si Synapse estuviera abierto, los dos se pelearían por ellas.

## Alternativas

- **Solo en THX, en vivo** (como las mejoras de salida): más simple, pero si THX no las guarda se pierden al reiniciar, y sin Synapse nadie las vuelve a poner. El usuario eligió el perfil.
- **Reenviarlas en cada sondeo:** cubriría cualquier pérdida, pero con Synapse abierto se pelearían. Detectar el reinicio del servicio alcanza para el caso real (reinicio de la PC o del servicio).
- **Guardarlas por fuera del perfil** (un solo ajuste global): no coincide con Synapse, donde cada perfil tiene las suyas.

## Consecuencias

- **Funcionan sin Synapse** (verificado de oído el 2026-09-26, escuchando el micrófono en vivo): los presets de EQ, la curva personalizada (−12 a +12 dB), la puerta de voz y su umbral, la normalización, la claridad vocal y la reducción de ruido.
- **Al abrir el panel o arrancar el proceso en segundo plano, el perfil manda.** Lo que se hubiera puesto desde Synapse se reemplaza por lo del perfil.
- **Falta confirmar la hipótesis** de que THX las olvida al reiniciar, y que rzr las repone. Ver "Esperando al usuario" en [ESTADO.md](../ESTADO.md).
- **La interfaz COM `IVSSrvSettings` se declara a mano**, con los tamaños de la biblioteca de tipos de `VSSrv` 3.2.3.0 (`float[10]` para la curva). Si THX la cambia, hay que revisarla, igual que `IVSSrvTHXSettings` ([ADR 0004](0004-thx-por-com.md)).
- **El importador de `.synapse4` todavía no trae** las mejoras del micrófono.

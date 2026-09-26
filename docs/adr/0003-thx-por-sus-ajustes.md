# 0003. THX se controla escribiendo sus ajustes, sin redistribuir su driver

- Estado: Aceptada. Cómo escribir los ajustes: [ADR 0004](0004-thx-por-com.md)
- Fecha: 2026-09-26

## Contexto

Bass Boost, Normalización, Claridad de voz y THX Spatial Audio no los hace el headset: los hace el motor THX, un efecto de audio de Windows (APO) que Synapse instala en la salida de los audífonos. El usuario compró el headset con THX y quiere usarlo sin depender de Synapse. El efecto queda instalado en Windows y corre dentro de `audiodg.exe`; lo que hace falta es poder cambiar sus ajustes.

## Decisión

- rzr **no incluye ni redistribuye** archivos de THX o Razer (DLL, drivers, instaladores). El efecto lo instala Synapse o el propio usuario, reinstalando su paquete de driver exportado con `pnputil`.
- rzr controlará THX **escribiendo los mismos ajustes que escribe Synapse**, en el lugar donde el efecto los lee.
- Ese lugar se averigua con la captura `tools/capturar-synapse.ps1 -Fase thx` y, si hace falta, con Process Monitor.
  - **Resultado (2026-09-25):** el estado completo está en JSON en el registro de la salida de los audífonos (`{d5e8f0ab-4de6-4d91-ab21-68868dda6a4a},6`), con una copia por preset de THX en `HKCU\Software\THX\SpatialAudio\UserState`. Synapse no lo escribe directamente: se lo pide al servicio de THX (`VSSrv.exe`, parte del paquete de driver) por ZeroMQ. Detalle en [HALLAZGOS.md](../HALLAZGOS.md#dónde-guarda-thx-sus-ajustes).
  - **Cómo escribir**, por orden de preferencia: la utilidad `spatial-config-util.exe` si acepta opciones; si no, los mismos mensajes que envía Synapse al servicio; como último recurso, el registro (requiere administrador).
- Mientras no se sepa, la pestaña MEJORAS explica qué es THX y cómo dejarlo funcionando (mejoras de audio activadas, Windows Sonic apagado).

## Alternativas

- **Redistribuir el driver de THX con rzr:** viola la licencia de Razer/THX; además rzr dejaría de ser un exe pequeño y portátil.
- **Hablarle directamente a la DLL de THX** (llamar a sus funciones internas): frágil. Cada versión de Synapse puede cambiarla, y requiere ingeniería inversa del binario.
- **Reemplazar THX por un efecto propio** (p. ej. Equalizer APO): posible para el EQ, pero no replica el sonido espacial de THX. Queda como idea aparte.

## Consecuencias

- **Instalación:** THX necesita que Synapse (o el paquete exportado) lo haya instalado alguna vez. Sin eso, rzr solo puede ofrecer las mejoras propias de Windows.
- **Cambios de THX:** si Razer cambia dónde guarda los ajustes, hay que repetir la captura.
- **Sin Synapse:** Bass Boost siguió sonando con Synapse cerrado y los servicios de Razer detenidos. El servicio de THX viene en el paquete de driver de THX, no en Synapse, así que depender de él no contradice esta decisión.
- **Pendiente:** confirmar si el efecto necesita el servicio de THX corriendo. Si hiciera falta un servicio **de Razer**, esta decisión se revisa.

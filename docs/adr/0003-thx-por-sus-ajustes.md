# 0003. THX se controla escribiendo sus ajustes, sin redistribuir su driver

- Estado: Aceptada (a la espera de la captura `-Fase thx`)
- Fecha: 2026-09-26

## Contexto

Bass Boost, Normalización, Claridad de voz y THX Spatial Audio no los hace el headset: los hace el motor THX, un efecto de audio de Windows (APO) que Synapse instala en la salida de los audífonos. El usuario compró el headset con THX y quiere usarlo sin depender de Synapse. El efecto queda instalado en Windows y corre dentro de `audiodg.exe`; lo que hace falta es poder cambiar sus ajustes.

## Decisión

- rzr **no incluye ni redistribuye** archivos de THX o Razer (DLL, drivers, instaladores). El efecto lo instala Synapse o el propio usuario, reinstalando su paquete de driver exportado con `pnputil`.
- rzr controlará THX **escribiendo los mismos ajustes que escribe Synapse**, en el lugar donde el efecto los lee.
- Ese lugar se averigua con la captura `tools/capturar-synapse.ps1 -Fase thx` y, si hace falta, con Process Monitor.
- Mientras no se sepa, la pestaña MEJORAS explica qué es THX y cómo dejarlo funcionando (mejoras de audio activadas, Windows Sonic apagado).

## Alternativas

- **Redistribuir el driver de THX con rzr:** viola la licencia de Razer/THX; además rzr dejaría de ser un exe pequeño y portátil.
- **Hablarle directamente a la DLL de THX** (llamar a sus funciones internas): frágil. Cada versión de Synapse puede cambiarla, y requiere ingeniería inversa del binario.
- **Reemplazar THX por un efecto propio** (p. ej. Equalizer APO): posible para el EQ, pero no replica el sonido espacial de THX. Queda como idea aparte.

## Consecuencias

- **Instalación:** THX necesita que Synapse (o el paquete exportado) lo haya instalado alguna vez. Sin eso, rzr solo puede ofrecer las mejoras propias de Windows.
- **Cambios de THX:** si Razer cambia dónde guarda los ajustes, hay que repetir la captura.
- **Pendiente:** confirmar si THX sigue sonando con Synapse cerrado y sus servicios detenidos (lo pregunta la captura). Si necesita un servicio de Razer, esta decisión se revisa.

# 0002. Solo secuencias de Synapse/OpenRazer, con candado y verificación por lectura

- Estado: Aceptada
- Fecha: 2026-09-26 (registra decisiones tomadas entre el 2026-09-25 y el 26)

## Contexto

El headset no tiene documentación pública. Sus comandos se conocen por capturas de Synapse y por el driver de OpenRazer, que los verificó en hardware. El firmware tiene trampas: el enlace descarta el primer frame tras estar inactivo; un cambio de familia de presets no llega al preset pedido; la curva se guarda en la ranura del preset activo, sea cual sea. Además, el panel y el proceso en segundo plano abren el dongle a la vez.

## Decisión

- rzr solo envía secuencias observadas en Synapse o verificadas por OpenRazer.
- Cada secuencia:
  - empieza con un frame de modo remoto que despierta el enlace;
  - corre dentro de un candado entre procesos (`Local\rzr_hid_bus`);
  - se confirma leyendo el estado cuando el firmware lo permite (preset activo antes de escribir una curva, reintentos al cambiar de familia).
- Juego, Película y Música son de solo lectura: el headset trae su curva de fábrica.
- Cuando una secuencia no se oye en el headset del usuario, las variantes se prueban con la **prueba guiada**, que pregunta al usuario y guarda el método que funciona. No se cambia la secuencia por defecto a ciegas.

## Alternativas

- **Escribir sin leer** (como la primera versión): más simple y rápido, pero puede escribir una curva en la ranura equivocada y dañar otro preset.
- **Explorar comandos nuevos desde el panel:** riesgo de dejar el headset en un estado raro sin forma de saber por qué.
- **Sin candado entre procesos:** una consulta de un proceso corta la escritura del otro (observado como escrituras ignoradas).

## Consecuencias

- **Más seguro, pero más lento:** un cambio de preset puede tardar 1–2 s por las lecturas y reintentos.
- **Pruebas:** cada comando nuevo necesita una fuente (captura u OpenRazer) y una prueba con sus bytes.
- **Cuando algo no funciona**, el camino es: registro de depuración, prueba guiada y comparación con OpenRazer ([HALLAZGOS.md](../HALLAZGOS.md#ecualizador)).

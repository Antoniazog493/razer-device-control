# Historial de cambios

Cambios que nota quien usa rzr. Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/); las versiones siguen [SemVer](https://semver.org/lang/es/). Los cambios nuevos van en "Sin publicar".

## [Sin publicar] — 0.2.0

### Agregado
- Panel de control con el estilo de Synapse: Sonido, Mejoras, Micrófono, Energía y Ajustes.
- Ecualizador: presets Juego/Película/Música, presets Esports (Apex, CoD, CS2, Fortnite, Valorant) y curva personalizada de 10 bandas que se arrastra.
- Sidetone, No molestar, apagado automático, batería, carga, estado del botón de silencio, firmware y número de serie.
- Volumen y silencio de Windows; dispositivo de salida y entrada predeterminados al conectar.
- Varios perfiles; importar perfiles de Synapse (`.synapse4`) con el botón o arrastrando el archivo.
- Registro de caídas del enlace (`conexion.log`) con resumen en ENERGÍA.
- Prueba guiada del ecualizador y registro de depuración (`debug.log`) en AJUSTES › DIAGNÓSTICO.
- Script de captura (`tools/capturar-synapse.ps1`) con fases para Synapse, las mejoras de Windows y THX.
- Documentación del proyecto: estado, arquitectura, hallazgos, reglas y decisiones.

### Cambiado
- El panel ahora es una página web en WebView2: `rzr.exe` pasa de 8,7 MB a ~1,3 MB y desaparecen los parpadeos en negro.
- La configuración se guarda en `%APPDATA%\rzr\config.json` (se migra sola desde el registro).
- Doble clic abre el panel; `--silent --watch` ya no muestra una consola.

### Corregido
- El comando que se creía "volumen" (`0x93`) es el selector de preset; `0x9D` es la familia del preset.
- El panel y el proceso en segundo plano ya no se interrumpen al usar el dongle a la vez.
- Una sola lectura perdida ya no cuenta como desconexión.

### Conocido
- La curva Personalizada se guarda en el headset pero todavía no se oye (en investigación; ver `docs/ESTADO.md`).

## [0.1.0] — 2026-03-16

### Agregado
- Primera versión: envía al headset el preset y la curva del ecualizador que Synapse enviaría.
- `--watch`: vigila el headset y aplica la configuración al conectar, con una sola instancia.
- `--silent` para el inicio de Windows; batería en la configuración.
- Dispositivo predeterminado de salida y entrada.

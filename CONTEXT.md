# Glosario

Términos del dominio de rzr. Úsalos igual en la interfaz, el código (su traducción entre paréntesis), los commits y la documentación. Si un término no está aquí o se usa con otro sentido, se aclara y se agrega antes de seguir.

## El hardware

**Headset** (headset): los audífonos Razer BlackShark V2 Pro. Guarda sus propios ajustes (presets, curvas, sidetone, etc.) y los conserva sin rzr.
_Evitar:_ "dispositivo" cuando se habla solo de los audífonos.

**Dongle** (dongle): el receptor USB de 2.4 GHz. rzr le habla al dongle y el dongle reenvía al headset.

**Enlace** (link): la conexión inalámbrica entre el dongle y el headset. Puede estar caído aunque el dongle esté conectado.

**Caída** (drop): un corte del enlace, con su hora y duración.

## Hablar con el headset

**Comando** (command): un mensaje de rzr al headset. Es una **consulta** (lee un valor) o una **escritura** (cambia un valor).

**Evento** (event): un mensaje que el headset envía por su cuenta, sin que se lo pidan: cambio de enlace, batería, botón EQ, botón de silencio.

**Secuencia** (sequence): la serie ordenada de comandos que hace falta para un cambio (por ejemplo, seleccionar un preset). El orden viene de Synapse y OpenRazer.

**Modo remoto** (remote mode): estado en el que el headset acepta comandos del PC. Se activa al empezar una secuencia.

**Aplicar** (apply): enviar al headset todo lo que dice el perfil activo.

## Ecualizador

**Curva** (curve): los 10 valores del ecualizador, de −5 a +5 dB, de 31 Hz a 16 kHz.

**Preset** (preset): un ajuste del ecualizador guardado en el headset. Cada preset tiene su **ranura**.
_Evitar:_ "preajuste" en el código (se usa solo en la interfaz, como sinónimo visible).

**Ranura** (slot): el lugar del headset donde se guarda la curva de un preset. Juego, Película y Música tienen ranuras de fábrica que no se pueden cambiar; Personalizado y los Esports sí.

**Familia** (family): el grupo al que pertenece un preset. **Estándar**: Juego, Película, Música y Personalizado. **Esports**: Apex Legends, Call of Duty, CS2, Fortnite y Valorant. El headset recuerda un preset por familia.

**Selector** (selector): el número con que el headset identifica cada preset.

**Personalizado** (custom): el preset cuya curva define el usuario.

**Método de envío del EQ** (EQ method): la variante de secuencia que usa rzr para que una curva nueva se oiga. La elige la prueba guiada.

## Ajustes del headset

**Sidetone** (sidetone): escuchar tu propia voz por los audífonos. En la interfaz: "Monitoreo de micrófono".

**No molestar** (dnd): bloquea las llamadas del celular por Bluetooth mientras se usa el dongle.

**Apagado automático** (auto off): minutos sin uso tras los que el headset se apaga (15 a 60).

## rzr

**Perfil** (profile): un conjunto con nombre de todos los ajustes del headset. Hay un **perfil activo**.

**Panel** (panel): la ventana de rzr.

**Proceso en segundo plano** (watcher): rzr corriendo sin ventana (`--silent --watch`), que aplica el perfil cada vez que el headset se conecta.

**Prueba guiada** (guided test, wizard): el asistente que envía curvas de prueba y pregunta al usuario si oye el cambio, para averiguar qué método de envío del EQ funciona.

**Captura** (capture): una sesión de `tools/capturar-synapse.ps1` que registra qué hace Synapse (o Windows) en cada paso.

## Windows y THX

**Mejoras de audio** (audio enhancements): los efectos que Windows aplica a un dispositivo de audio. Si están desactivadas, ningún efecto (tampoco THX) se aplica.

**Efecto de audio** (APO): un componente que procesa el sonido dentro de Windows, no en el headset. THX Spatial Audio es uno.

**Motor THX** (THX): el efecto de audio que instala Synapse. Hace Bass Boost, Normalización, Claridad de voz y el sonido espacial. No es parte del headset.
_Evitar:_ "driver de THX" para referirse a sus ajustes; el driver es solo el paquete que lo instala.

**Windows Sonic** (Windows Sonic): el sonido espacial propio de Windows. Choca con THX.

**Preset de THX** (THX preset): un ajuste del ecualizador por software de THX: `Game Mode`, `Cinema Mode`, `Music Mode` o `Custom`. Es distinto del preset del headset, aunque Synapse elige los dos a la vez.
_Evitar:_ decir solo "preset" cuando se habla de THX.

**Estado de THX** (THX state): el JSON con todos los ajustes del motor THX (espacial, Bass Boost, normalización, claridad de voz, preset de THX, curva) que queda en el registro de la salida de los audífonos.

**Servicio de THX** (THX service): `VSSrv.exe`, instalado con el paquete de driver de THX. Recibe los cambios de Synapse (por ZeroMQ) y guarda el estado de THX. No es un servicio de Razer.

# Reglas de trabajo

Cómo se trabaja en rzr. Valen para personas y para asistentes de IA. Si una regla estorba de verdad, se cambia aquí primero, con su motivo.

## 1. Principios

1. **Primero no hacer daño al headset.** Solo se envían secuencias que ya usa Synapse o que OpenRazer verificó en hardware. Nada de probar comandos "a ver qué pasa" desde el panel.
2. **Decir la verdad sobre el estado.** Lo probado en el headset real es distinto de lo que solo compila. [ESTADO.md](ESTADO.md) usa ✅ / 🟡 / ❌ / ⏳ y nunca se marca ✅ sin prueba real.
3. **Simple antes que ingenioso.** Un archivo claro vale más que una abstracción que se usa una sola vez.
4. **La interfaz muestra lo que el headset dice**, no lo que esperamos: si una escritura no se confirma, se avisa.
5. **Pequeño y verificable.** Cambios cortos, cada uno probado y con su commit.

## 2. Idiomas y nombres

- **Español:** textos de la interfaz, documentación, mensajes al usuario, registros (`conexion.log`, `debug.log`).
- **Inglés:** código, identificadores, comentarios, mensajes de commit.
- Los términos del dominio salen de [CONTEXT.md](../CONTEXT.md). Si falta uno, se agrega ahí antes de usarlo.

## 3. Código

- **Formato:** `cargo fmt` (configurado en `rustfmt.toml`). El CI lo exige.
- **Lint:** `cargo clippy --all-targets -- -D warnings` sin avisos. Si un aviso no tiene sentido, se desactiva en el lugar exacto con un comentario que diga por qué.
- **Comentarios:** explican el *porqué* (una peculiaridad del firmware, una decisión), no repiten lo que el código ya dice. Cada archivo empieza con `//!` diciendo de qué se encarga.
- **Errores:** nada de `unwrap()` sobre datos del headset, archivos o Windows. Los errores llegan al usuario en español y a `debug.log` con detalle.
- **Capas:** respeta [ARQUITECTURA.md](ARQUITECTURA.md). `protocol.rs` sin E/S; la E/S del headset solo en el hilo del headset; la página no conoce el protocolo.
- **Headset:** toda secuencia nueva:
  - toma el candado del bus;
  - empieza con el frame de modo remoto que despierta el enlace;
  - confirma leyendo cuando el firmware lo permite;
  - deja constancia en `debug.log` (`dlog!`).
- **Dependencias:** solo si ahorran trabajo real y son livianas. Antes de agregar una, mide cuánto crece `rzr.exe` y anótalo en el commit.
- **Página (`ui/`):** HTML, CSS y JavaScript simples, sin frameworks ni paso de compilación. Todo lo que cambie el estado pasa por un comando a Rust. Si cambian los comandos o el estado, se actualiza `demo.js`.

## 4. Pruebas y verificación

Antes de cada push, todo en verde:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
node --check ui/app.js
cargo build --release --target x86_64-pc-windows-gnu
```

- **Protocolo:** cada comando nuevo tiene una prueba que compara sus bytes con una captura real.
- **Lógica sin hardware** (config, importador, prueba guiada, comandos de la página): prueba unitaria.
- **Interfaz:** abrir `ui/index.html` en un navegador y, si se puede, `rzr --demo`.
- **Hardware:** lo que no se pueda probar aquí va a "Esperando al usuario" en ESTADO.md, con pasos concretos para probarlo.
- **Arreglos:** primero reproducir el fallo (una prueba que falla, o un paso a paso) y después arreglarlo.

## 5. Git

- **Ramas:** el trabajo va en una rama (hoy `UI-creation`), nunca directo en `main`.
- **Commits:** en inglés, título en imperativo de menos de ~60 caracteres ("Add THX phase to the capture script"). El cuerpo dice el porqué, qué se verificó y qué no.
- **Un commit, una idea.** El formateo masivo va aparte de los cambios de lógica.
- **Pull requests:** usar la plantilla (`.github/pull_request_template.md`). El CI debe estar en verde.
- **Nunca se suben** binarios de Razer/THX, capturas con datos personales (números de serie completos, rutas de usuario) ni `debug.log` sin revisar.

## 6. Documentación

| Cuándo | Qué actualizar |
|---|---|
| Siempre | [ESTADO.md](ESTADO.md) |
| El usuario nota el cambio | [CHANGELOG.md](../CHANGELOG.md) (sección "Sin publicar") |
| Cambia la estructura, un hilo, un archivo en disco | [ARQUITECTURA.md](ARQUITECTURA.md) |
| Se averigua algo del headset, Synapse, THX o Windows | [HALLAZGOS.md](HALLAZGOS.md), con su fuente |
| Aparece o cambia un término | [CONTEXT.md](../CONTEXT.md) |
| Decisión difícil de revertir, con alternativas reales | un ADR nuevo en [adr/](adr/) |
| Cambia el uso o el protocolo | [README.md](../README.md) |

## 7. Terminado significa

Un cambio está terminado cuando:

- el CI está en verde;
- se probó hasta donde se puede sin el headset, y lo que falta está en ESTADO.md como ⏳ con pasos para el usuario;
- la documentación de la tabla anterior está al día;
- el commit explica qué se verificó.

## 8. Al empezar y al cerrar una sesión

- **Al empezar:** leer [ESTADO.md](ESTADO.md) y los últimos commits (`git log --oneline -10`).
- **Al cerrar:** dejar ESTADO.md al día ("Sigue" en orden y "Esperando al usuario" con pasos claros), todo commiteado y subido. Una sesión nueva debe poder continuar sin el historial del chat.

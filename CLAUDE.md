# rzr: guía para trabajar en este repositorio

Panel de control para los audífonos Razer BlackShark V2 Pro (dongle `1532:0555`) que reemplaza a Razer Synapse. Rust + una página web (WebView2) en Windows.

## Antes de empezar

1. Lee **[docs/ESTADO.md](docs/ESTADO.md)**: qué está hecho, qué falta y qué sigue. Es la fuente de verdad del trabajo.
2. Usa los términos de **[CONTEXT.md](CONTEXT.md)** (preset, ranura, curva, modo remoto…) en código, commits y conversación.
3. Sigue **[docs/REGLAS.md](docs/REGLAS.md)**. Lo más importante está abajo.

**Si trabajas en la PC del usuario (Windows, headset real):** lee también [docs/HANDOFF.md](docs/HANDOFF.md).

Más documentos: [docs/ARQUITECTURA.md](docs/ARQUITECTURA.md) (cómo está construida la app), [docs/HALLAZGOS.md](docs/HALLAZGOS.md) (lo averiguado sobre el headset, Synapse y THX), [docs/adr/](docs/adr/) (decisiones y sus porqués), [CHANGELOG.md](CHANGELOG.md).

## Comandos

```
cargo fmt --check                          # formato (rustfmt.toml: 120 columnas)
cargo clippy --all-targets -- -D warnings  # sin avisos
cargo test                                 # pruebas
node --check ui/app.js                     # sintaxis de la página
cargo build --release --target x86_64-pc-windows-gnu   # comprobar Windows desde Linux
cargo run -- --demo                        # panel con un headset simulado
```

En Linux hacen falta `libwebkit2gtk-4.1-dev` (panel) y, para compilar a Windows, el target `x86_64-pc-windows-gnu` con `mingw-w64`. La página se puede abrir sola en un navegador (`ui/index.html`, carga `ui/demo.js`).

## Reglas clave

- **Idiomas:** textos de la interfaz, documentación y mensajes al usuario en español; código, comentarios, commits y nombres en inglés.
- **Nunca escribir al headset sin verificar:** toda secuencia nueva sigue lo capturado de Synapse / OpenRazer, pasa por el candado del bus (ver `device.rs`) y se confirma leyendo el estado cuando el firmware lo permite.
- **Distinguir lo verificado en hardware de lo que no.** Nada se marca "funciona" en ESTADO.md sin prueba en el headset real; lo demás es "sin confirmar".
- **Cada cambio actualiza la documentación:** ESTADO.md siempre, CHANGELOG.md si el usuario lo nota, ARQUITECTURA.md si cambia la estructura, un ADR si es una decisión difícil de revertir.
- **Nada de archivos de Razer/THX en el repo** (DLL, drivers, instaladores): solo se documenta cómo obtenerlos.
- **Antes de subir:** formato, clippy, pruebas y compilación para Windows en verde.

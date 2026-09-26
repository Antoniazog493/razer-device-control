# 0001. La interfaz es una página web en WebView2

- Estado: Aceptada
- Fecha: 2026-09-26

## Contexto

El panel se dibujaba con egui. Con OpenGL mostraba cuadros negros de vez en cuando en Windows; al pasarlo a Direct3D 12 (wgpu), `rzr.exe` creció de 5,8 a 8,7 MB. Modelar la interfaz en código Rust era lento, y la app es un panel básico que va a crecer (THX, mejoras de Windows, quizá otros modelos). Criterios: gratis, liviano, fácil de diseñar y de ampliar.

## Decisión

La interfaz es una página web (`ui/`: HTML, CSS y JavaScript sin frameworks) mostrada con el WebView del sistema: WebView2 en Windows, mediante `wry` y `tao`. Rust es el controlador: le envía a la página el estado completo como JSON y recibe comandos JSON (`Msg`). La página va incluida en el exe.

## Alternativas

- **Seguir con egui + Direct3D:** funciona, pero 8,7 MB y la interfaz sigue siendo difícil de modelar.
- **egui solo con OpenGL:** menos peso, pero vuelven los cuadros negros.
- **Slint con dibujo por CPU:** gratis y declarativo, pero en una prueba el exe quedó en ~8,5 MB (datos Unicode e imágenes que siempre incluye).
- **Controles nativos de Win32:** el exe más chico, pero mucho trabajo y difícil lograr el estilo Synapse.
- **WebView2** (elegida): exe de ~1,3 MB, se diseña con HTML/CSS, se puede ver en cualquier navegador con datos de demo y no usa la GPU por su cuenta.

## Consecuencias

- **Peso y GPU:** `rzr.exe` baja a ~1,3 MB y desaparecen los problemas de GPU propios.
- **RAM:** con el panel abierto usa unos 80–120 MB (Edge). El proceso en segundo plano no carga la página y no paga ese costo.
- **Requisito:** WebView2 viene con Windows 10 actualizado y Windows 11; si falta, rzr avisa con el enlace del instalador.
- **Contrato:** la página y Rust se comunican por un contrato (`Msg` y `App::view`) que hay que mantener, y `demo.js` debe seguirlo.
- **Linux:** para desarrollar en Linux hace falta WebKitGTK (`libwebkit2gtk-4.1-dev`).

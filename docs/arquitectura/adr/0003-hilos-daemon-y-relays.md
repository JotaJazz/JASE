# 0003. Trabajo lento fuera del hilo de la UI, sin frenar el cierre

- Estado: Aceptado (actualizado en 2.0.0)
- Fecha: 2026-10-19

## Contexto
Descargas (imágenes de mapas, base de países, actualizaciones) y lecturas de disco
pueden colgarse (DNS lento, unidad de red). Un trabajo que el cierre tuviera que esperar
retendría la app.

## Decisión
La red corre en tareas de tokio y el disco y las descargas en `spawn_blocking`
(`app::blocking` en `rust/jase/src/app/mod.rs`). El resultado vuelve a `update` como
`Message` de un `Task` o una `Subscription` de Iced, con su generación o número de pedido;
`update` descarta el que no coincide.

## Consecuencias
Cerrar la ventana termina el proceso sin esperar a una descarga ni a un escaneo. `update` y
`view` nunca hacen red ni disco (salvo guardar la config, pequeña).

Actualizado en 2.0.0: en 1.3.0 eran hilos daemon con un relay QObject sin padre y los
escaneos un `QThread` con espera acotada al cerrar.

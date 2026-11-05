# 0003. Trabajo lento en hilos daemon con relays sin padre

- Estado: Aceptado
- Fecha: 2026-10-19

## Contexto
Descargas (imágenes de mapas, base de países, actualizaciones) y lecturas de disco
pueden colgarse (DNS lento, unidad de red). Un hilo no daemon o un `QThread` con padre
retendría el cierre de la app.

## Decisión
Ese trabajo corre en hilos daemon (`ui/daemon.py`, `ui/*_loader.py`). El resultado
vuelve por la señal de un relay QObject **sin padre**, creado una vez en el hilo de la
UI; cada ventana conecta su slot y descarta resultados tardíos por identidad o
generación.

## Consecuencias
Cerrar nunca espera a una descarga. Emitir con la app cerrándose se protege con
`RuntimeError`. Los escaneos (red con asyncio) siguen en `QThread` con espera acotada al
cerrar (`CLOSE_WAIT_MS`).

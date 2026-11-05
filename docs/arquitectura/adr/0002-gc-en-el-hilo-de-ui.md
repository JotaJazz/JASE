# 0002. Recolección de basura solo en el hilo de la UI

- Estado: Aceptado
- Fecha: 2026-10-27

## Contexto
El recolector automático puede liberar un ciclo con wrappers de Qt en cualquier hilo que
reserve memoria (p. ej. el bucle asyncio de un worker) y destruir un QObject fuera de su
hilo: en Windows es una «access violation».

## Decisión
`ui/gc_guard.py` apaga el recolector automático y recolecta con un QTimer del hilo de la
UI cada 500 ms, con los mismos umbrales por generación.

## Consecuencias
`MainThreadGC` vive mientras dure `main()`. Ningún hilo debe llamar a `gc.collect()`.

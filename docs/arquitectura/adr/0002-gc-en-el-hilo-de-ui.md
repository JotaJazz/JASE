# 0002. Recolección de basura solo en el hilo de la UI

- Estado: Reemplazado: no aplica en Rust (2.0.0)
- Fecha: 2026-10-27

## Contexto
El recolector automático de Python podía liberar un ciclo con wrappers de Qt en cualquier
hilo que reservara memoria (p. ej. el bucle asyncio de un worker) y destruir un QObject fuera
de su hilo: en Windows era una «access violation».

## Decisión
En 1.3.0, un guardián apagaba el recolector automático y recolectaba con un QTimer del hilo
de la UI cada 500 ms, con los mismos umbrales por generación.

## Consecuencias
Desde la 2.0.0 la app es Rust con Iced: no hay recolector ni objetos de Qt que destruir en
otro hilo, y la regla desaparece. El código de 1.3.0 queda en la tag `v1.3.0`.

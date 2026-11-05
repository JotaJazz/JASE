# Notas de arquitectura (normativas)

Resumen; la versión completa y la matriz de dependencias: [límites](../arquitectura/limites.md).

## Debe
- Parsers en `src/jase/protocols/` puros: bytes → datos o `ProtocolError`.
- Red en `QThread` + asyncio; disco y descargas en hilos daemon con relay sin padre.
- Resultados tardíos descartados por generación o identidad de la petición.
- Texto remoto como texto plano o escapado; argumentos al juego validados.
- Columnas de la tabla por `COL_*`; colores como tokens en todos los temas.
- Cada texto visible con `tr()`/`tr_n()` y traducido en `src/jase/locales/` (5 catálogos).
- Masters iguales en `masters.json`, `src/jase/assets/masters.json` y `src/jase/games.py`.

## No debe
- PySide6, keyring o Pillow en `cli.py` ni en lo que importa.
- Red, DNS, carpetas de juegos, imágenes o archivos grandes en el hilo de la UI.
- `@Slot` en los mixins `src/jase/ui/window_*.py`.
- `gc.collect()` fuera del hilo de la UI.
- Enviar la clave de Steam a otro host que `api.steampowered.com`.
- Reescribir rutas guardadas por el usuario o versionar archivos de juegos.

## Decisiones irreversibles
Índice y estado de cada una: [adr/README.md](../arquitectura/adr/README.md).

# 0004. Lista oficial de masters remota + empaquetada; gana la más nueva

- Estado: Aceptado
- Fecha: 2026-10-19

## Contexto
Los masters cambian o mueren; publicar una versión por cada cambio es lento. El usuario
también quiere añadir o desactivar masters propios.

## Decisión
`masters.json` (raíz del repo, copia idéntica en `src/jase/assets/`) se descarga una vez
al día desde GitHub, validado estrictamente. Se usa la más nueva por fecha `updated`
entre la caché y la empaquetada. Los cambios del usuario van aparte, en
`masters.custom.json` (añadidos y desactivados).

## Consecuencias
Las tres fuentes (raíz, empaquetada y `games.py`) deben coincidir (`tests/test_masters.py`)
y la fecha `updated` sube con cada cambio. Un `masters.json` remoto inválido se ignora.

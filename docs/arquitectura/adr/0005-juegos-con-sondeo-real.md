# 0005. Un juego entra solo con un sondeo real de su master

- Estado: Aceptado
- Fecha: 2026-10-18

## Contexto
Muchas listas de masters públicas están muertas o mezclan juegos; añadir un juego sin
comprobarlo deja entradas vacías y protocolos adivinados.

## Decisión
Un juego (o master) entra solo si su master devuelve servidores en un sondeo real
(`scripts/probe_masters.py`) y los servidores responden. Las respuestas reales se
guardan como capturas en `tests/fixtures/*.bin` y los tests se escriben contra ellas.

## Consecuencias
Los descartados se documentan con fecha (README, «Discarded in probing»). Nunca se
versionan archivos de los juegos.

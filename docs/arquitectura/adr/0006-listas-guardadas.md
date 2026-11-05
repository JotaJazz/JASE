# 0006. Listas de servidores guardadas entre sesiones

- Estado: Aceptado
- Fecha: 2026-11-05

## Contexto
Como en el ASE original, al abrir se quiere ver la última lista para entrar directo a un
servidor conocido, sin esperar un escaneo.

## Decisión
Cada escaneo completo sin error se guarda en `cache/servers/<juego>.json` (más
`index.json` con conteos y fechas), escrito en orden por un solo hilo daemon. Al elegir
un juego sin lista en memoria se lee la guardada en un hilo y se muestra con su fecha; no
se refresca solo al abrir. Archivos dañados se ignoran (se escanea).

## Consecuencias
Búsqueda de jugadores, «Ahora se juega» y avisos no usan listas del disco (datos viejos).
Las listas LAN no se guardan. Cambiar «vacíos de Steam» olvida las de Steam.

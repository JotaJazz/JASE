# Notas de arquitectura (normativas)

Resumen; la versión completa y la matriz de dependencias: [límites](../arquitectura/limites.md).

## Debe
- Parsers en `rust/jase-core/src/protocols/` puros: bytes → datos o `ProtocolError`, sin pánicos.
- Red en tareas de tokio; disco y descargas en `spawn_blocking`; los resultados vuelven a
  `update` como `Message`.
- Resultados tardíos descartados por generación o número de pedido.
- Texto remoto como texto plano; argumentos al juego validados (argv, nunca un shell).
- Columnas de la tabla por `enum Column`; colores como campos de `Palette` en todos los temas.
- Cada texto visible con `tr`/`tr_fmt`/`tr_n` y traducido en `rust/jase-core/locales/` (5 catálogos).
- Masters iguales en `masters.json` y `rust/jase-core/src/games_table.rs`.

## No debe
- Iced, keyring o imágenes en `jase-core` (salvo la feature `map-images`).
- Red, DNS, carpetas de juegos, imágenes o archivos grandes en `update`/`view`.
- `unwrap`/`panic` sobre datos remotos.
- Enviar la clave de Steam a otro host que `api.steampowered.com`.
- Reescribir rutas guardadas por el usuario o versionar archivos de juegos.

## Decisiones irreversibles
Índice y estado de cada una: [adr/README.md](../arquitectura/adr/README.md).

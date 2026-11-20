# 0001. Núcleo sin UI (`jase-core`); la app es otro crate

- Estado: Aceptado (actualizado en 2.0.0)
- Fecha: 2026-10-28

## Contexto
Consultar servidores desde scripts o servidores sin pantalla no debería exigir la interfaz,
el llavero ni las bibliotecas de imágenes, que pesan e incluso fallan sin entorno gráfico.

## Decisión
`protocols`, `scanner` y el núcleo viven en el crate `jase-core`, que no depende de Iced,
keyring ni imágenes en sus features por defecto (las imágenes de mapas, con la feature
`map-images`). `jase-cli` usa solo `jase-core`; la app de escritorio es el crate `jase`.

## Consecuencias
Todo código con Iced o el llavero vive en `rust/jase/`. `rust/jase/tests/deps.rs` falla si
`cargo tree -p jase-core` trae Iced, keyring o `image`.

Actualizado en 2.0.0: en 1.3.0 (Python) el núcleo usaba solo la biblioteca estándar, la app
era el extra `gui` (PySide6, keyring, Pillow) y lo vigilaba un test de la CLI.

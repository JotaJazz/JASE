# 0001. Núcleo sin Qt; la app es el extra `gui`

- Estado: Aceptado
- Fecha: 2026-10-28

## Contexto
Consultar servidores desde scripts o servidores sin pantalla no debería exigir PySide6,
keyring ni Pillow, que pesan e incluso fallan sin entorno gráfico.

## Decisión
`protocols/`, `scanner.py` y el núcleo usan solo la biblioteca estándar. `pip install .`
instala `jase-cli`; la app de escritorio es el extra `gui`. `python -m jase` sin PySide6
explica cómo instalarlo y termina.

## Consecuencias
Todo código con Qt vive en `ui/`. `tests/test_cli.py` falla si la CLI carga PySide6,
keyring o Pillow. Excepción: `ui/favorites.py`, lógica pura sin Qt usada por el núcleo.

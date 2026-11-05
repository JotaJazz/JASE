# Guía de desarrollo

Comandos: [comandos.md](../desarrollo/comandos.md). Recetas:
[agregar un juego](../desarrollo/agregar-juego.md), [publicar](../desarrollo/publicar.md).

## Flujo
1. Rama desde `master` (`fix/…`, `feat/…`); nunca trabajar directo en `master`.
2. Test que falla primero, luego el cambio mínimo; red y disco fuera del hilo de la UI.
3. Antes de cada commit: `pytest -q -W error`, `ruff check src tests scripts`, `mypy`.
4. Merge sin fast-forward (`git merge --no-ff`), suite verde sobre el resultado.

## Commits
En español, con prefijo de tipo y ámbito: `fix(ui): …`, `feat(juegos): …`,
`refactor(scanner): …`, `docs: …`, `build: versión X; …`. Identificadores en inglés.

## Sincronía documentación ↔ código
Un cambio que deja una doc incorrecta la corrige en el mismo commit. Cada trabajo
terminado agrega una etapa a [PROJECT_MEMORY.md](PROJECT_MEMORY.md).

| Si cambias… | Actualiza… |
|---|---|
| un juego, master o familia | README (×2), [SYSTEM_SUMMARY](SYSTEM_SUMMARY.md), [protocolos](../arquitectura/protocolos.md), `packaging/release-notes.md` |
| un módulo, capa o import entre capas | [visión general](../arquitectura/vision-general.md), [límites](../arquitectura/limites.md) |
| una decisión de arquitectura | un ADR nuevo en [adr/](../arquitectura/adr/README.md) (el viejo pasa a «Reemplazado») |
| una caché, hilo o vista de la UI | [ui.md](../arquitectura/ui.md) |
| un comando o script | [comandos.md](../desarrollo/comandos.md) |
| la salida JSON de `jase-cli` | [cli.md](../cli.md) (y `SCHEMA` si es incompatible) |
| algo dejado a propósito o sin verificar en Windows | [deuda](../hoja-de-ruta/deuda.md) |
| la versión | [publicar.md](../desarrollo/publicar.md), [SYSTEM_SUMMARY](SYSTEM_SUMMARY.md) |

El verificador (`scripts/check_docs.py`, corre en los tests) señala enlaces rotos, rutas
inexistentes, datos personales y etapas de la memoria con más de 3 viñetas. Las IPs de
ejemplo van en 192.0.2.x, 198.51.100.x o 203.0.113.x; las rutas de usuario, con
marcadores (`<CARPETA_DEL_JUEGO>`, `%APPDATA%`).

## Plantilla de etapa de la memoria

```markdown
## AAAA-MM-DD — tema
- Qué cambió y por qué importa.
- Qué se descartó (y por qué).
- Qué quedó abierto (enlace a la deuda si aplica).
```

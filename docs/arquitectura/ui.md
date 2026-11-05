# UI (PySide6)

`src/jase/ui/main_window.py` define `MainWindow`, que hereda de 21 mixins
`src/jase/ui/window_*.py`, uno por función (acciones, favoritos, filtros, amigos,
RCON, bandeja, listas guardadas…). Cada mixin documenta en su docstring qué atributos
espera de `MainWindow`; mypy los revisa sin `attr-defined` (lista en `pyproject.toml`,
vigilada por `tests/test_packaging_meta.py`).

## Piezas

| Pieza | Archivo | Qué hace |
|---|---|---|
| Tabla | `server_model.py` | `ServerTableModel` (filas, `COL_*`), `ServerFilterProxy` (filtros rápidos, versión, texto, marcas del árbol), delegados de ping y ocupación |
| Árbol de juegos | `game_tree.py` | juegos, mods, mapas y países con marcas que filtran la tabla |
| Ficha | `server_card.py` | datos del servidor, imagen del mapa, historial de ocupación |
| Workers de red | `scan_worker.py` | `ScanWorker`, `QueryWorker`, `DetectWorker`, `RconWorker`: `QThread` + asyncio |
| Hilos daemon | `daemon.py` (`run_daemon`), `map_loader.py`, `country_loader.py`, `exe_icon_loader.py`, `masters_loader.py`, `update_loader.py`, `update_dialog.py`, `window_saved_lists.py` | disco y descargas fuera del hilo de la UI |
| Vigilancias | `friend_watch.py`, `slot_watch.py` | consultas periódicas de favoritos, recientes y servidores llenos |
| Tema | `theme.py`, `icons.py`, `flags.py` | tokens de color por tema, hojas de estilo, iconos |
| Instancia única | `single_instance.py` | `QLocalServer`: enlaces `jase://` y «mostrar» a la JASE abierta |
| GC | `gc_guard.py` | recolección de basura solo en el hilo de la UI |

## Listas y cachés

| Caché | Dónde | Se invalida |
|---|---|---|
| Lista de la sesión por juego (`_cache`) | memoria | escaneo completo nuevo; cambio de «vacíos de Steam» |
| Lista guardada entre sesiones | `cache/servers/` (`server_cache.py`, `window_saved_lists.py`) | se reemplaza en cada escaneo completo |
| Exe instalado por juego (`_installed_cache`) | memoria, la llena `ExeIconLoader` | nueva respuesta del hilo; sin ruta = no instalado |
| País por IP (`_countries`) | memoria | al vaciar la tabla o recargar la base |
| Imágenes de mapas | `cache/maps/` | nunca (archivos por juego y mapa) |

Una lista es «completa» solo tras un escaneo terminado sin error o restaurada de una
caché (`_tree_prune_ok`): solo entonces reemplaza la guardada o poda marcas del árbol.

## Vistas

- **Lista normal** del juego (master), **LAN** (`scan_lan`) y **vistas directas**
  Favoritos / Recientes (`_direct_view`), que consultan sus direcciones sin master y no
  aplican filtros rápidos ni marcas del árbol.
- **Refresco:** F5 vacía y reescanea; el automático (`_auto_refresh`) actualiza en su
  sitio y al final quita los que no respondieron.
- **Diálogos no modales:** «Ahora se juega» (`now_playing.py`), búsqueda de jugadores
  (`player_search.py`), RCON (`rcon_dialog.py`). Las listas leídas del disco no cuentan
  para ellos.

## i18n

`tr()`/`tr_n()` de `src/jase/i18n.py`; la clave es el texto en español y las
traducciones viven en `src/jase/locales/` (en, pt, fr, de, it). Lo vigilan
`tests/test_i18n.py` y `tests/test_i18n_ui.py`.

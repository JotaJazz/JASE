# UI (Iced)

`rust/jase/src/lib.rs` arranca la app con `iced::daemon` (la ventana principal y las
herramientas en ventanas propias). El estado, `update` y `view` viven en `App`
(`rust/jase/src/app/mod.rs`), repartido por dominio en `rust/jase/src/app/` (escaneo,
favoritos, filtros, amigos, RCON, bandeja, vigilancias, tareas de fondo…): cada archivo
maneja sus `Message`. Los widgets y diálogos que no dependen de `App` están en
`rust/jase/src/` y se prueban sin ventana.

## Piezas

| Pieza | Archivo (en `rust/jase/src/`) | Qué hace |
|---|---|---|
| Tabla | `table/model.rs`, `table/filter.rs`, `table/view.rs` | filas y `enum Column`, filtro (rápidos, versión, texto, marcas del árbol), vista virtualizada con barra de ocupación y ping |
| Árbol de juegos | `tree.rs` | juegos, mods, mapas y países con marcas que filtran la tabla |
| Ficha | `card.rs`, `details.rs`, `chart.rs` | datos del servidor, imagen del mapa, jugadores y reglas, historial de ocupación |
| Red y disco | `scan.rs`, `app/background.rs`, `app/mod.rs` (`blocking`) | escaneo, consulta, detección y RCON como `Task` de tokio; disco y descargas en `spawn_blocking` |
| Vigilancias | `app/watch.rs`, `app/friends.rs` | consultas periódicas de favoritos, recientes y servidores llenos |
| Tema | `theme/palette.rs`, `theme/style.rs`, `icons.rs`, `flags.rs` | `Palette` por tema, estilos de los 5 aspectos, iconos y banderas |
| Colores `^N` | `colored.rs` | nombres con códigos de color de Quake/CoD partidos en tramos |
| Menús y controles | `menu.rs`, `widgets.rs` | menús dibujados y la casilla, el combo, el campo y el tooltip en los 5 estilos |
| Diálogos | `settings.rs`, `find_games.rs`, `add_server.rs` | Ajustes, «Buscar juegos», «Agregar servidor» |
| Instancia única | `single_instance.rs` | socket Unix o *pipe* con nombre: enlaces `jase://` y «mostrar» a la JASE abierta |
| Bandeja | `tray/` | `ksni` en Linux, `Shell_NotifyIconW` en Windows |

## Listas y cachés

Ubicación de lo que se guarda en disco: [visión general](vision-general.md#datos-que-se-guardan).

| Caché | Se invalida |
|---|---|
| Lista de la sesión por juego (`App::cache`, memoria) | escaneo completo nuevo; cambio de «vacíos de Steam» |
| Lista guardada entre sesiones (`server_cache.rs` de `jase-core`, `saved_lists.rs`) | se reemplaza en cada escaneo completo |
| Exe instalado por juego (`App::installed`, memoria, con su generación) | nueva respuesta de la tarea; sin ruta = no instalado |
| País por IP (memoria) | al vaciar la tabla o recargar la base |
| Imágenes de mapas | nunca (archivos por juego y mapa) |

Una lista es «completa» solo tras un escaneo terminado sin error o restaurada de una
caché (`tree_prune_ok`): solo entonces reemplaza la guardada o poda marcas del árbol.

## Vistas

- **Lista normal** del juego (master), **LAN** (`scan_lan`) y **vistas directas**
  Favoritos / Recientes (`app/direct.rs`), que consultan sus direcciones sin master y no
  aplican filtros rápidos ni marcas del árbol.
- **Refresco:** F5 vacía y reescanea; el automático (`app/auto_refresh.rs`) actualiza en
  su sitio y al final quita los que no respondieron.
- **Ventanas aparte:** «Ahora se juega» (`now_playing.rs`), búsqueda de jugadores
  (`player_search.rs`), RCON (`rcon_console.rs`). Las listas leídas del disco no cuentan
  para ellas.

## i18n

`tr`/`tr_fmt`/`tr_n` de `rust/jase-core/src/i18n.rs`; la clave es el texto en español y las
traducciones viven en `rust/jase-core/locales/` (en, pt, fr, de, it; se editan a mano). Lo
vigilan `rust/jase-core/tests/i18n.rs` y `rust/jase/tests/i18n_literals.rs`.

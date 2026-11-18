# Deuda, riesgos y pendientes

Lo que se sabe y se dejó a propósito. Al resolver algo, se quita de aquí (y se anota en
la [memoria](../ai/PROJECT_MEMORY.md)).

## Sin verificar en Windows real

Desde Linux no se lanzan juegos de Windows; esto solo se probó con tests.

| Qué | Desde |
|---|---|
| Exes y argumentos de lanzamiento: JKA, SoF2, BF1942 (con `+game`), CS 1.6, JK2, Xonotic, Warsow, Warfork, Tremulous, Smokin' Guns (`+connect`), D-Day (`+set game dday` con `quake2.exe`, `q2pro.exe`, `r1q2.exe` o `yquake2.exe`) (vale igual para el port a Rust) | 1.0 |
| Unirse a Tactical Ops (`host:port?password=`) y encontrarlo por las carpetas `Infogrames` y `TacticalOps` | 1.2 |
| JKA y JK2 de Steam (variable `SteamAppId`) y ET: Legacy por su clave del registro | 1.1 |
| Claves del registro y carpetas de Steam de «Buscar juegos» | 1.0 |
| Iconos de los exes | 1.0 |
| Extracción de levelshots de los .pk3 | 1.0 |
| Zandronum (`-iwad … -connect`) | 1.0 |
| Difusión LAN a través del firewall de Windows (vale igual para el port a Rust) | 1.0 |
| RCON contra servidores reales (vale igual para el port a Rust) | 1.0 |
| Actualización del zip portátil completa (JASE abierta + descarga real); el script `.cmd` sí se probó con cmd.exe (vale igual para el port a Rust) | 1.2.1 |
| Abrir juegos con Wine desde Linux | 1.0 |
| Segunda JASE abierta sin enlace: «mostrar» a la ventana minimizada en la bandeja o tapada; Windows puede solo hacer parpadear el botón (falta `AllowSetForegroundWindow`) | 2026-11-05 |
| Port a Rust: escaneo con servidores caídos (ICMP de puerto cerrado como `ConnectionReset` en el socket compartido); probado solo en el CI de Windows | 2026-11-17 |
| Port a Rust, fase 4b: lectura del registro, ICMP de puerto cerrado (`ConnectionReset`) en LAN y RCON y el script portátil con `cmd.exe`/`robocopy` se prueban solo en el CI de Windows. *Junctions* en la detección: sin test. Difusión LAN real: sin test, solo la lista de direcciones (`broadcast_addresses()`; los tests de LAN inyectan 127.0.0.1). Las comparaciones con Python de la 4b (`normpath`, `shlex`, script del actualizador, zip escrito por Python) solo corren en local: ningún job del CI tiene `.venv`. Compilar para Windows no se verifica en local (`ring` necesita mingw) | 2026-11-17 |
| Port a Rust, fase 4c: las comparaciones con Pillow (`bcn`, `thumbnail_size`) y con Python (nombres de caché y de GameTracker, `countries_json_matches_python`) solo corren donde hay `.venv` (no en el CI; los nombres de países los cubre igual el golden). Reemplazar el PNG de la caché con el destino abierto en Windows (devuelve el archivo que ya está) no tiene test en Windows; el de carpeta de solo lectura es solo Unix | 2026-11-17 |

## Deuda aceptada

| Qué | Dónde | Cuándo revisarlo |
|---|---|---|
| Config bloqueada por otro proceso: no se copia a `.bad` y `save_config` la pisa al cerrar | `src/jase/config.py` (`_keep_damaged`) | si aparece en la práctica |
| Si falla la copia del exe en la actualización portátil, puede quedar a medias | `src/jase/portable_update.py` (`updater_script`) | si aparece en la práctica |
| Con la primera JASE colgada, la segunda sale sin abrir ventana (sin acuse de recibo) | `src/jase/ui/single_instance.py` | si se reporta |
| «Solo instalados» no se entera si se instala o desinstala un exe con la ruta ya guardada (hasta cambiar rutas o reiniciar) | `src/jase/ui/main_window.py` (`_tree_game_ids`) | si molesta |
| Cada juego recorre su carpeta dos veces en el hilo (`resolve_exe` + `resolve_game_exe`) | `src/jase/ui/exe_icon_loader.py` | unidades muy lentas |
| El relay de cada `ExeIconLoader` no tiene padre y puede destruirse en el hilo daemon | `src/jase/ui/exe_icon_loader.py` | si aparece un aviso de Qt |
| Quitar filas al final del auto-refresco es O(k·n) | `src/jase/ui/main_window.py` (`_on_worker_finished`) | listas grandes que cambian mucho |
| Listas guardadas: cerrar justo tras un escaneo puede perder ese guardado (en Rust solo si el disco se traba más de 2 s: el cierre espera al escritor hasta 2 s); un `.tmp` de un cierre a medias queda en `cache/servers/` | `src/jase/ui/window_saved_lists.py`, `rust/jase/src/saved_lists.rs` | si el directorio crece |
| `index.json` dañado: las listas guardadas de los demás juegos no se cargan (se escanea) | `src/jase/server_cache.py` | aceptado por diseño |
| Contador del árbol tras un F5 detenido con lectura del disco en camino | `src/jase/ui/main_window.py` (`_update_sidebar`) | cosmético |
| RCON: frases de «clave incorrecta» sin capturas de SoF2 y otros motores | `src/jase/rcon.py` (`quake`) | con capturas reales |
| Port a Rust: la paridad de `jase-cli` con la CLI Python solo corre donde hay `.venv` (el job de Rust del CI no lo tiene y la salta) | `rust/jase-cli/tests/parity_python.rs` | al tocar `jase-cli`, correrla en local |
| Port a Rust, fase 5a (2026-11-18): `smoke_exits_ok` (`jase --smoke`) solo corre con pantalla o `xvfb-run`; sin `DISPLAY` ni `WAYLAND_DISPLAY` se salta con aviso. Las celdas largas se cortan sin «…» (Iced 0.14 no lo hace) hasta la 5d. Los juegos de Steam avisan que falta la clave hasta la 5f (llavero). Al empezar la 5d, el escaneo pasa a un subestado `ScanState` con su propia generación. El cierre ignora el `window::Id`: filtrarlo al pasar a `iced::daemon` en la 5i | `rust/jase/tests/smoke.rs`, `rust/jase/src/table/`, `rust/jase/src/app.rs` | 5d, 5f y 5i |
| Port a Rust, fase 5b (2026-11-18): divergencias aceptadas con 1.3.0 (barras de desplazamiento sin flechas, trama ni estrías; línea discontinua de Terminal continua; esquinas de los biseles rectas; sin barra visible, la selección de época no llega al borde). Los SVG (iconos, banderas, flechas, icono de la app) se incrustan con `include_bytes!` desde `src/jase/assets/`. El ancho de «00/00» de la barra por bloques es estimado (avances de DejaVu Sans y DejaVu Sans Mono), no medido: con Tahoma en Windows puede correrse el marco. El modo claro/oscuro del sistema en vivo solo se probó en WSLg | `rust/jase/src/icons.rs`, `rust/jase/src/flags.rs`, `rust/jase/src/table/view.rs` (`players_text_width`, `cell_vm`) | fase 7: mudar los assets a una carpeta `assets` dentro de `rust/jase` antes de borrar `src/`; probar el modo del sistema en KDE y Windows; medir «00/00» si la captura de Windows no alinea |
| Port a Rust, fase 5c (2026-11-18): la fecha de «lista guardada del …» sale en UTC (`AAAA-MM-DD HH:MM`, sin crate de fechas ni formato de `QLocale`); el icono del exe en el árbol queda para la 5e (hoy, la insignia). La lista que abre el combo *País* no lleva banderas (`pick_list` solo dibuja texto); los nombres largos del árbol se cortan sin «…» al medio; el botón *Filtros guardados* es un `pick_list` (ancho de la opción más larga, texto a la izquierda, menú plano). El error de la base de países queda fijo en la barra de estado (Qt lo borra a los 8 s). F5 en un juego sin masters o sin clave de Steam pierde las marcas del árbol (paridad con Python). Divergencias aceptadas de las capturas: `docs/superpowers/capturas/divergencias.md` (local) | `rust/jase/src/saved_lists.rs` (`short_datetime`), `rust/jase/src/widgets.rs`, `rust/jase/src/presets.rs`, `rust/jase/src/app.rs` (`on_country_db`) | 5d: menú de filtros guardados y recorte; 5e: icono del exe y hora local |
| Port a Rust: el job de Rust del CI compila Iced en frío (sin caché) | `.github/workflows/` | el job tiene 60 min (Windows se estima 18–28 min en frío); si se acerca al límite al crecer la app (5e–5i): agregar caché de cargo |
| Port a Rust: cancelar la descarga del instalador puede tardar hasta 30 min si el servidor se cuelga a mitad del cuerpo (`ureq` 3 no tiene plazo por lectura; Python, 10 s) | `rust/jase-core/src/app_update.rs` (`download_installer`) | fase 5: la UI no espera la tarea cancelada |

## Pendiente

| Qué | Estado |
|---|---|
| Port a Rust ([diseño](port-rust.md)) | fases 1 a 4, 5a, 5b y 5c hechas; sigue la 5d, rama `rust`; la deuda de config bloqueada de Python vale igual para `rust/jase-core/src/config.rs` |
| Port a Rust, fase 5a: prueba del dueño en Linux nativo con KDE Plasma y Wayland (sin WSL). Correr desde `rust/` `cargo run --release -p jase`, escanear CoD2, ordenar por varias columnas y desplazar 1.000+ filas con la rueda, la barra y Re Pág/Av Pág. Mirar que la ventana abra (sin «Broken pipe» ni otro error de Wayland), que el desplazamiento no dé tirones y que cerrar la ventana termine el proceso enseguida. Si falla, el respaldo X11 (`force_x11` también cuando Wayland no abre) entra como tarea aparte antes de la 5c | pendiente (decisión del 2026-11-15) |
| Port a Rust, fase 5b: mirar las capturas lado a lado (`docs/superpowers/capturas/`, locales; mejor en Windows, con Tahoma) y decidir la insignia apagada del juego elegido (en el árbol de 1.3.0 también sale apagada). De la 5c: aceptar o pedir un widget propio para la lista del combo *País* sin banderas, el botón *Filtros guardados* hecho con `pick_list` y el recorte sin «…» al medio | pendiente del dueño |
| Juegos nuevos: Deus Ex, Unreal, Sauerbraten, AssaultCube, Red Eclipse, DDNet (sondeos del 2026-11-05 positivos) | plan escrito, sin ejecutar |
| Rune, Postal 2, STVEF, Alien Arena, QuakeWorld | masters muertos (sondeo 2026-11-05); no entran |

## Fuera de alcance (decidido)

- QuakeWorld: masters UDP muertos y hubapi.quakeworld.nu inalcanzable.
- Halo CE: el `enctypeX` solo existe con licencia GPL.

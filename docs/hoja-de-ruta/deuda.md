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
| Cada juego recorre su carpeta dos veces en el hilo (`resolve_exe` + `resolve_game_exe`; en Rust, `game_installed` y el icono en el mismo `blocking`) | `src/jase/ui/exe_icon_loader.py`, `rust/jase/src/scan.rs`, `rust/jase/src/exe_icon.rs` | unidades muy lentas |
| El relay de cada `ExeIconLoader` no tiene padre y puede destruirse en el hilo daemon | `src/jase/ui/exe_icon_loader.py` | si aparece un aviso de Qt |
| Quitar filas al final del auto-refresco es O(k·n) | `src/jase/ui/main_window.py` (`_on_worker_finished`) | listas grandes que cambian mucho |
| Listas guardadas: cerrar justo tras un escaneo puede perder ese guardado (en Rust solo si el disco se traba más de 2 s: el cierre espera al escritor hasta 2 s, más hasta 2 s del historial); un `.tmp` de un cierre a medias queda en `cache/servers/` | `src/jase/ui/window_saved_lists.py`, `rust/jase/src/saved_lists.rs` | si el directorio crece |
| `index.json` dañado: las listas guardadas de los demás juegos no se cargan (se escanea) | `src/jase/server_cache.py` | aceptado por diseño |
| Contador del árbol tras un F5 detenido con lectura del disco en camino | `src/jase/ui/main_window.py` (`_update_sidebar`) | cosmético |
| RCON: frases de «clave incorrecta» sin capturas de SoF2 y otros motores | `src/jase/rcon.py` (`quake`) | con capturas reales |
| Port a Rust: la paridad de `jase-cli` con la CLI Python solo corre donde hay `.venv` (el job de Rust del CI no lo tiene y la salta) | `rust/jase-cli/tests/parity_python.rs` | al tocar `jase-cli`, correrla en local |
| Port a Rust, fase 5a (2026-11-18): `smoke_exits_ok` (`jase --smoke`) solo corre con pantalla o `xvfb-run`; sin `DISPLAY` ni `WAYLAND_DISPLAY` se salta con aviso. Los juegos de Steam avisan que falta la clave hasta la 5f (llavero). El cierre ignora el `window::Id`: filtrarlo al pasar a `iced::daemon` en la 5i | `rust/jase/tests/smoke.rs`, `rust/jase/src/app/` | 5f y 5i |
| Port a Rust, fase 5b (2026-11-18): divergencias aceptadas con 1.3.0 (barras de desplazamiento sin flechas, trama ni estrías; línea discontinua de Terminal continua; esquinas de los biseles rectas; sin barra visible, la selección de época no llega al borde). Los SVG (iconos, banderas, flechas, icono de la app) se incrustan con `include_bytes!` desde `src/jase/assets/`. El ancho de «00/00» de la barra por bloques es estimado (avances de DejaVu Sans y DejaVu Sans Mono), no medido: con Tahoma en Windows puede correrse el marco. El modo claro/oscuro del sistema en vivo solo se probó en WSLg | `rust/jase/src/icons.rs`, `rust/jase/src/flags.rs`, `rust/jase/src/table/view.rs` (`players_text_width`, `cell_vm`) | fase 7: mudar los assets a una carpeta `assets` dentro de `rust/jase` antes de borrar `src/`; probar el modo del sistema en KDE y Windows; medir «00/00» si la captura de Windows no alinea |
| Port a Rust, fase 5c (2026-11-18): la fecha de «lista guardada del …» sale como `AAAA-MM-DD HH:MM` en hora local (sin formato de `QLocale`). La lista que abre el combo *País* no lleva banderas (`pick_list` solo dibuja texto); los nombres largos del árbol se cortan sin «…» al medio. F5 en un juego sin masters o sin clave de Steam pierde las marcas del árbol (paridad con Python). Divergencias aceptadas de las capturas: `docs/superpowers/capturas/divergencias.md` (local) | `rust/jase/src/saved_lists.rs` (`short_datetime`), `rust/jase/src/widgets.rs` | si el dueño lo pide |
| Port a Rust, fase 5d (2026-11-18): las celdas largas de la tabla se cortan sin «…» (Iced 0.14 no lo hace; medir cada celda visible en cada `view` cuesta ~120 `Paragraph`). Los menús no se recorren con flechas ni tienen mnemónicos, y en ellos no se elige con el botón derecho (Qt sí). Mientras se arrastra un borde de columna o el divisor, el cursor vuelve a puntero o a texto sobre botones y campos (`mouse_area::interaction` solo vale si el hijo no pide otro). *Acerca de* cierra con «Cerrar» (Qt: «OK» del `QMessageBox`). En Wayland la posición de la ventana no se guarda (el sistema no la da). `Ctrl+Q` solo existe fuera de Windows y eso solo lo prueba el CI de Windows. Divergencias aceptadas de las capturas (barra de progreso plana de 18 px, bisel pulsado de 98/VGUI solo en el botón de tema, atajos con teclados no latinos, desfase al arrastrar una columna, separador de Terminal continuo): `docs/superpowers/capturas/divergencias.md` (local) | `rust/jase/src/table/view.rs`, `rust/jase/src/menu.rs`, `rust/jase/src/bars.rs`, `rust/jase/src/window_state.rs` | recorte y flechas del menú: widget propio si el dueño lo pide; atajos con `physical_key` si alguien usa otra distribución |
| Port a Rust, fase 5d (2026-11-18): la ventana restaurada solo se achica al tamaño del monitor actual y se centra solo si su posición es imposible (no toca el monitor actual ni dos monitores a cada lado, cada uno del tamaño del actual y al menos de 2560×1440). Iced 0.14 no da la lista de monitores ni el origen del actual, así que una ventana guardada en un monitor ya desconectado pero «posible» (p. ej. x = 2500 sin segundo monitor) abre fuera de la pantalla; también se ajusta al monitor entero, no al área de trabajo (Qt usa `availableGeometry`). El 0×0 y la posición (-32000, -32000) que Windows informa al minimizar se ignoran | `rust/jase/src/window_state.rs` (`fit`) | cuando Iced exponga los monitores (o con la API del sistema por el handle de `window::run`): comprobar contra todos y usar el área de trabajo |
| Port a Rust, fase 5e (2026-11-18): las tablas de jugadores y reglas no seleccionan filas (Qt resalta la fila y no hace nada más). En empates del orden de jugadores Rust deja el orden del servidor (Qt, el del orden anterior). Los bordes de acento de *rust* y *brown* van sin radio; el asa de los divisores es transparente (en 98, Qt la pinta `button_top`) y sin la trama de puntos de Fusion. El mínimo del `pane_grid` (alto de la ficha) sube también el del panel de la tabla. Con el borde de acento de 3 px o el doble de Terminal, Qt deja el recuadro del mapa de 120 px y lo recorta abajo; Rust lo achica con su borde entero. Un servidor que deja de responder con el mismo mapa conserva la imagen (como Python). La primera muestra de historial que llega antes de leer el archivo se pierde (a lo sumo una por servidor). En Windows la hora local usa las reglas de horario de verano actuales; los iconos de exe monocromos quedan con la insignia; el icono del exe y la hora local en Windows solo los prueba el CI de Windows (fuera de Windows, la insignia, como Python). Divergencias de las capturas: `docs/superpowers/capturas/divergencias.md` (local) | `rust/jase/src/card.rs`, `rust/jase/src/details.rs`, `rust/jase/src/window_state.rs`, `rust/jase/src/exe_icon.rs`, `rust/jase/src/localtime.rs`, `rust/jase/src/app/history.rs` | icono y hora: si el CI o la prueba en Windows fallan; el resto, si el dueño lo pide |
| Port a Rust: el job de Rust del CI compila Iced en frío (sin caché) | `.github/workflows/` | el job tiene 60 min (Windows se estima 18–28 min en frío); si se acerca al límite al crecer la app (5e–5i): agregar caché de cargo |
| Port a Rust: cancelar la descarga del instalador puede tardar hasta 30 min si el servidor se cuelga a mitad del cuerpo (`ureq` 3 no tiene plazo por lectura; Python, 10 s) | `rust/jase-core/src/app_update.rs` (`download_installer`) | fase 5: la UI no espera la tarea cancelada |

## Pendiente

| Qué | Estado |
|---|---|
| Port a Rust ([diseño](port-rust.md)) | fases 1 a 4 y 5a a 5e hechas; sigue la 5f, rama `rust`; la deuda de config bloqueada de Python vale igual para `rust/jase-core/src/config.rs` |
| Port a Rust, fase 5a: prueba del dueño en Linux nativo con KDE Plasma y Wayland (sin WSL). Correr desde `rust/` `cargo run --release -p jase`, escanear CoD2, ordenar por varias columnas y desplazar 1.000+ filas con la rueda, la barra y Re Pág/Av Pág. Mirar que la ventana abra (sin «Broken pipe» ni otro error de Wayland), que el desplazamiento no dé tirones y que cerrar la ventana termine el proceso enseguida. Si falla, el respaldo X11 (`force_x11` también cuando Wayland no abre) entra como tarea aparte antes de la 5c | pendiente (decisión del 2026-11-15) |
| Port a Rust, fase 5b: mirar las capturas lado a lado (`docs/superpowers/capturas/`, locales; mejor en Windows, con Tahoma) y decidir la insignia apagada del juego elegido (en el árbol de 1.3.0 también sale apagada). De la 5c: aceptar o pedir un widget propio para la lista del combo *País* sin banderas y el recorte sin «…» al medio. De la 5d: celdas largas sin «…», menús sin flechas, y el radio marcado de los menús planos (1.3.0 pinta un cuadrado dentro del círculo; Rust, un punto redondo). De la 5e: jugadores y reglas sin selección de filas, borde doble de Terminal como uno simple de 3 px, el recuadro del mapa achicado en vez de recortado bajo los bordes gruesos, el asa de los divisores sin la trama de puntos de Fusion y 🔒 sin color si la letra no lo trae | pendiente del dueño |
| Juegos nuevos: Deus Ex, Unreal, Sauerbraten, AssaultCube, Red Eclipse, DDNet (sondeos del 2026-11-05 positivos) | plan escrito, sin ejecutar |
| Rune, Postal 2, STVEF, Alien Arena, QuakeWorld | masters muertos (sondeo 2026-11-05); no entran |

## Fuera de alcance (decidido)

- QuakeWorld: masters UDP muertos y hubapi.quakeworld.nu inalcanzable.
- Halo CE: el `enctypeX` solo existe con licencia GPL.

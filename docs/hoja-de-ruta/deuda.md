# Deuda, riesgos y pendientes

Lo que se sabe y se dejó a propósito. Al resolver algo, se quita de aquí (y se anota en
la [memoria](../ai/PROJECT_MEMORY.md)).

## Sin verificar en Windows real

Desde Linux no se lanzan juegos de Windows; esto solo se probó con tests.

| Qué | Desde |
|---|---|
| Argumentos de lanzamiento: JKA, SoF2, CS 1.6, BF1942, JK2, Xonotic, Warsow, Warfork, Tremulous, Smokin' Guns, D-Day, unirse a Tactical Ops | README, «Supported games» |
| Zandronum (`-iwad … -connect`) | 1.0 |
| Difusión LAN a través del firewall de Windows | 1.0 |
| RCON contra servidores reales | 1.0 |
| Actualización del zip portátil completa (JASE abierta + descarga real); el script `.cmd` sí se probó con cmd.exe | 1.2.1 |
| Abrir juegos con Wine desde Linux | 1.0 |
| Segunda JASE abierta sin enlace: «mostrar» a la ventana minimizada en la bandeja o tapada; Windows puede solo hacer parpadear el botón (falta `AllowSetForegroundWindow`) | 2026-11-05 |

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
| Listas guardadas: cerrar justo tras un escaneo puede perder ese guardado; un `.tmp` de un cierre a medias queda en `cache/servers/` | `src/jase/ui/window_saved_lists.py` | si el directorio crece |
| `index.json` dañado: las listas guardadas de los demás juegos no se cargan (se escanea) | `src/jase/server_cache.py` | aceptado por diseño |
| Contador del árbol tras un F5 detenido con lectura del disco en camino | `src/jase/ui/main_window.py` (`_update_sidebar`) | cosmético |
| RCON: frases de «clave incorrecta» sin capturas de SoF2 y otros motores | `src/jase/rcon.py` (`quake`) | con capturas reales |

## Pendiente

| Qué | Estado |
|---|---|
| Juegos nuevos: Deus Ex, Unreal, Sauerbraten, AssaultCube, Red Eclipse, DDNet (sondeos del 2026-11-05 positivos) | plan escrito, sin ejecutar |
| Rune, Postal 2, STVEF, Alien Arena, QuakeWorld | masters muertos (sondeo 2026-11-05); no entran |

## Fuera de alcance (decidido)

- QuakeWorld: masters UDP muertos y hubapi.quakeworld.nu inalcanzable.
- Halo CE: el `enctypeX` solo existe con licencia GPL.

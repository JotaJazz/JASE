# Port a Rust (diseño)

- Estado: Aprobado
- Fecha: 2026-11-15
- Rama: `rust`

## Intención

Reescribir JASE completa en Rust: núcleo, CLI y app de escritorio. Lo que se busca:

- **Rendimiento:** escaneos y tabla con miles de servidores sin tirones.
- **Ejecutable liviano:** un binario sin Python ni Qt, en lugar del paquete de PyInstaller.
- **Distribución simple:** un solo archivo por plataforma, sin instalar dependencias.
- **Lenguaje moderno:** tipos fuertes y concurrencia segura.

El usuario final no debería notar el cambio, salvo porque la app es más liviana y más
rápida.

## Decisiones del dueño (2026-11-15)

- **Aspecto:** lo más parecido posible a la app Qt actual: mismos menús, atajos, columnas,
  diálogos, temas (incluidos Win98, XP y Luna) e iconos. La lista de control es
  `docs/guia/guia-usuario.md` más capturas de la versión 1.3.0 lado a lado.
- **Paso de 1.3 a 2.0:** por el actualizador automático actual (`src/jase/app_update.py`,
  `src/jase/portable_update.py`), con limpieza de los archivos de PyInstaller.
- **Python congelado en 1.3.0** durante el port. Un arreglo urgente en Python obliga a
  llevarlo a Rust con su test en el mismo trabajo.
- **Wayland:** dentro de WSL se fuerza X11. En Linux nativo (KDE Plasma con Wayland) se
  prueba al inicio de la fase 5; si falla, X11 por XWayland como respaldo.

## Metas

- Paridad completa con la versión Python 1.3.0: los 36 juegos y las 6 familias, y todo lo
  que hoy hace la UI (detalle en `docs/ai/SYSTEM_SUMMARY.md` y `docs/guia/guia-usuario.md`).
- Los mismos contratos: salida JSON de `jase-cli` (schema 1, `docs/cli.md`), enlaces
  `jase://`, `masters.json`, listas guardadas (schema 1) y exportación de favoritos.
- Leer la config, los favoritos, las notas, las contraseñas (keyring) y las listas
  guardadas que deja la versión Python, en las mismas rutas
  (`%APPDATA%\JimmysASE\`, `~/.config/jimmys-ase/`). Quien actualiza no pierde nada.
- Volver de 2.0 a 1.3 también funciona: Rust conserva los campos que no conoce,
  incluidos los blobs de Qt de `ui_state`, y escribe en un formato que Python lee.
  *(decisión del diseñador)*
- Mismas plataformas que 1.3.0: Windows 10/11 y Linux (base Ubuntu 22.04). Dibujo por
  software (tiny-skia) como respaldo si no hay GPU usable. *(decisión del diseñador)*
- Las reglas de `docs/arquitectura/limites.md` siguen valiendo, traducidas a Rust (abajo).

## No metas

- Juegos nuevos, rediseño de la UI o funciones nuevas durante el port. Lo pendiente de
  `docs/hoja-de-ruta/deuda.md` se hace después, ya en Rust.
- Mantener las dos versiones en paralelo a largo plazo.

## Anti-metas (sería un fracaso aunque funcione)

- Que un usuario pierda favoritos, contraseñas o config al pasar de la versión Python.
- Que un servidor malicioso pueda tumbar el escaneo o inyectar algo en la UI o en la
  línea de comandos del juego.
- Que la UI se congele por red o disco.
- Un ejecutable que necesite instalar runtimes o bibliotecas aparte.

## Enfoques considerados (decidido: Iced)

| Opción | A favor | En contra |
|---|---|---|
| **Iced 0.14** (elegido) | Modelo de mensajes (`update`/`view`) que calza con «red fuera del hilo de la UI» vía `Task`; aspecto pulido; Rust puro | La API cambia entre versiones; la tabla de servidores hay que escribirla (ver abajo) |
| egui | `TableBuilder` virtualizado listo; mucha documentación | Repinta todo en cada cuadro; estilo de herramienta |
| Slint, Tauri, imgui-rs, cxx-qt | — | Segundo lenguaje o licencia (Slint), webview (Tauri), C++ (imgui-rs), sigue con Qt pesado (cxx-qt) |

**Prototipo (2026-11-15):** la `iced::widget::table` incluida arma todas las filas (5.000
filas: 300 MB, filas de alto variable, sin encabezado fijo ni orden). Una tabla
virtualizada escrita a mano (~150 líneas: alto fijo, solo filas visibles, encabezado
fijo, orden por clic, selección, colores `^`) usó 156 MB, se sintió fluida en WSLg y el
binario quedó en 16 MB sin símbolos. En WSLg, Wayland corta la conexión («Broken pipe»);
con X11 (`WAYLAND_DISPLAY=` vacío) funciona.

## Estructura

Workspace de Cargo en `rust/` dentro del mismo repo. Mientras dure el port, Python sigue
en `src/` como referencia y para comparar comportamientos:

```
rust/
├── Cargo.toml          workspace
├── jase-core/          sin UI: protocolos, escáner, juegos, config, masters, launcher, mapas, i18n
├── jase-cli/           binario jase-cli (mismo JSON que hoy)
└── jase/               binario de la app (Iced)
```

- `jase-core` no depende de Iced, keyring ni imágenes. Es la regla de `cli.py` hoy (ADR
  0001), y la vigila un test o `cargo tree`. Las contraseñas (keyring) y las imágenes de
  mapas van en módulos con su propio *feature* o en `jase/`.
- Los datos compartidos no se duplican: `masters.json` (raíz y `src/jase/assets/`),
  banderas, iconos y temas SVG se leen desde su ubicación actual mientras exista Python, y
  se mueven a `rust/` cuando se retire.

## Reglas traducidas

| Regla actual | En Rust |
|---|---|
| Red en `QThread` con asyncio | `tokio` (UDP y HTTP) corriendo fuera del hilo de la UI; los resultados vuelven como `Message` vía `Task`/`Subscription` de Iced |
| Hilos daemon que no bloquean el cierre | tareas de tokio o `spawn_blocking`; al cerrar la ventana el proceso sale sin esperarlas |
| Resultados tardíos descartados por generación o identidad | cada pedido lleva un número de generación en su `Message`; `update` descarta el que no coincide |
| Datos remotos validados en el parser | parsers puros `&[u8] -> Result<ServerInfo, _>` con límites de largo y rango; sin `unwrap`/`panic` en datos remotos; tests de *fuzzing* para cada familia |
| Texto remoto como texto plano | Iced no interpreta marcado; los colores `^` se convierten a *spans* sin interpretar nada más |
| Argumentos del juego validados | `is_mod_name`/`is_wad_name` portados; el juego se lanza con `std::process::Command` (argv, nunca shell) |
| `COL_*` | `enum Column`, nunca índices sueltos |
| Tokens de color por tema | `struct Palette` con un campo por token; cada tema la llena completa (falta un campo = no compila) |
| `tr()`/`tr_n()` con clave en español, 5 catálogos | mismas claves y mismas traducciones, convertidas desde `src/jase/locales/*.py` a un formato que Rust lea en compilación; un test verifica que cada clave tenga las 5 traducciones |
| Masters iguales en 3 lugares | igual mientras viva Python; después, `masters.json` + `games.rs` |
| Juego nuevo solo con sondeo real y capturas | sin cambios; los tests de Rust usan las mismas `tests/fixtures/*.bin` |

## Orden de trabajo

Cada fase deja algo que se puede probar y termina con sus tests verdes.

1. **Núcleo de protocolos:** las 6 familias + huffman + steamweb, con los tests de
   parsers contra `tests/fixtures/`. **Hecha (2026-11-15).** Divergencias deliberadas,
   todas sin servidor en los dos lados o con Python cayéndose con una excepción no
   prevista:
   - GameSpy: `player_01` sin `player_1` (Python: `KeyError`; Rust: descarta el
     servidor); un `queryid` que no cabe en 32 bits (Python espera paquetes que nunca
     completan; Rust rechaza el paquete); en la lista del master, un puerto de más de 4300
     dígitos con ceros delante (Python: `ValueError`; Rust: lo lee).
   - API de Steam: un puerto en dígitos no ASCII se salta (Python puede caerse). Rust no
     lee JSON en UTF-16/32, con `NaN` ni con más de 128 niveles de anidamiento (Python
     sí); Steam responde UTF-8 plano. Los escapes `\uD800`–`\uDFFF` sueltos y el BOM sí
     se aceptan, como en Python.
2. **Escáner + juegos + masters**, en dos partes:
   - **2a (sin red): tabla de juegos, traducción y listas de masters. Hecha
     (2026-11-15).** La tabla se generó desde `games.py` (`scripts/gen_games_rs.py`);
     los comentarios de cada juego siguen en `games.py` y hay que llevarlos a mano al
     retirar Python. Divergencia deliberada: `tr` con un `{campo}` sin su argumento deja
     el texto tal cual (Python lanza `KeyError`).
   - **2b: escáner (consultas UDP en paralelo, masters, API web de Steam) y la
     descarga diaria de `masters.json`. Hecha (2026-11-17).** Un escaneo es un solo
     future `Send` de tokio (sin `tokio::spawn`): cancelar es soltarlo. HTTP con `ureq`
     en `spawn_blocking`. `SIO_UDP_CONNRESET` no se porta: en Windows, el ICMP de puerto
     cerrado llega al socket compartido como `ConnectionReset` y el lector ignora los
     errores de `recv_from` y sigue leyendo (lo prueba el CI de Windows, verde desde el 2026-11-19; no una máquina real). Divergencia
     deliberada: un `GameDef` sin `gamespy_gamename`/`valve_appid` da un error de master
     `protocol` en lugar del `ValueError` de Python; y `scan` descarta el error de
     `query_servers` (p. ej. el bind del socket) donde Python lo propaga.
3. **`jase-cli`:** paridad del JSON: igual al parsearlo y con las claves en el mismo
   orden (no byte a byte). Se valida comparando con la CLI Python 1.3.0 sobre los mismos
   datos. *(decisión del diseñador)* **Hecha (2026-11-17).** `rust/jase-cli` porta
   `cli.py` y `geo.py` (sin los nombres de países);
   `rust/jase-cli/tests/parity_python.rs` corre los dos binarios contra los mismos masters y servidores falsos (`games`, `scan` con y sin
   `--country`, sin clave de Steam, master muerto, juego desconocido; en inglés y en
   español) y compara JSON, stderr y código de salida, sin `jase`, `duration_s` ni
   `ping_ms`. No encontró diferencias. Divergencias deliberadas: el texto de `--help` y
   `--version` (clap frente a argparse) y la redacción de los mensajes genéricos de uso
   (opción desconocida, falta un argumento): los de clap, en inglés como los de argparse
   y con el mismo prefijo `jase-cli: ` y código 2. Los propios de JASE usan las claves
   de `cli.py`; la paridad compara el de juego desconocido en los dos idiomas.
   También: `-V` además de `--version`; `--timeout nan` y `1_0` se rechazan (Python 1.3.0 toma `nan`
   como tope instantáneo); `"jase"` informa la versión del crate (`0.1.0`) hasta la fase 7; un error
   de escritura en stdout da código 1 (Python: traceback).
4. **Resto del núcleo**, en tres partes:
   - **4a (datos persistentes): config, favoritos, notas, grupos, recientes,
     importación/exportación, listas guardadas, historial de ocupación, amigos, alertas de
     partida, búsqueda de jugadores, desglose del árbol y enlaces `jase://`. Hecha
     (2026-11-17).** Módulos de `jase-core`: `pyjson`, `config` (con `extra` para las
     claves que no conoce), `filter_presets`, `favorites`, `recent`, `favorites_io`,
     `server_cache`, `history`, `friends`, `party`, `player_search`, `browse`, `jase_url` y
     `masters::migrate_config_masters`. Los goldens que escribe Python 1.3.0 están en
     `rust/jase-core/tests/golden/data/` (`scripts/core_golden.py`): Rust los relee y los
     reguarda con los mismos bytes, y Python lee la config que escribe Rust. `passwords.py`
     (keyring) pasa a la fase 5. Divergencias deliberadas (Python 1.3.0 nunca escribe estos
     casos):
     - Claves desconocidas de la config: Python las descarta; Rust las conserva (`extra`) y
       las escribe después de las conocidas. Python las ignora al leer.
     - `NaN`/`Infinity` hacen inválido el archivo entero (config a `.bad`; Python los lee);
       los enteros fuera de `i64` cuentan como inválidos; el JSON en UTF-16/32 no se lee; los
       surrogates sueltos vuelven como `U+FFFD`.
     - Anidamiento: `pyjson` declara inválido un JSON de más de 128 niveles (tope propio);
       Python solo falla en su límite de recursión (~1000). Una config anidada entre 129 y
       ~1000 niveles carga en Python y en Rust queda dañada (valores por defecto y `.bad`).
     - En `game_paths` y `version_filter` un valor que no es texto descarta la entrada
       (Python guarda `str(v)`); en `masters` de la config solo vale `[texto, entero]`
       (Python acepta lo que pase `int(p)`).
     - En las listas guardadas el puerto solo con dígitos ASCII y un `ping_ms` negativo vale
       0 (`ServerInfo.ping_ms` es `u32`).
   - **4b: rutas (`paths.py`), detección de juegos, launcher, `identify`, LAN, RCON y
     actualización (`app_update`, `portable_update`). Hecha (2026-11-17).** Módulos de
     `jase-core`: `paths`, `detect` (con `winreg`, solo `cfg(windows)`), `launcher`,
     `identify`, `lan`, `rcon` (más `pystr::latin1_encode_lossy` y `py_splitlines`),
     `app_update`, `portable_update` y `games::Family::ALL`. Crates nuevos: `windows-sys`
     (solo Windows, para leer el registro sin `reg.exe` ni otro crate), `ring` (SHA-256 del
     instalador; ya lo compilaba rustls) y `zip` (sin features por defecto, solo deflate con
     `flate2`; escribir un lector de zip propio no compensa). Su costo es mayor que el de
     `sha2` (que se evitó): trae `typed-path`, `getrandom` 0.4, `constant_time_eq`, `nix`
     (solo Unix no-Linux), `rustversion` (solo build) y activa `zeroize/derive`
     (`zeroize_derive` y un segundo `syn` 2.x junto al 3.x). En Windows los nombres
     inválidos (`?*"<>|`, punto/espacio final, `CON`) no se sanean como en el `extractall` de
     Python: fallan con `Io` (el zip ya pasó el SHA-256 de la misma release). Lo de disco y procesos es
     bloqueante (la fase 5 lo llama con `spawn_blocking`); LAN y RCON son futures `Send`.
     El código solo de Windows (registro, `cmd.exe`/`robocopy`) lo prueba el CI de Windows.
     El abridor de URL por defecto (`steam://` y la página de la release) pasa a la fase 5.
     Divergencias deliberadas:
     - Las *junctions* de Windows cuentan como enlaces y la detección no las recorre
       (Python sí).
     - `broadcast_addresses`: `255.255.255.255` y el `.255` (/24) de la IP con ruta por
       defecto y, solo en Windows, de las IPv4 de `COMPUTERNAME`; «privada» es
       `is_private() || is_link_local()` (Python cuenta además las redes de documentación).
     - `is_mod_name`/`is_wad_name` solo ASCII: el `IGNORECASE` de Python acepta además `ſ`,
       `ı` y `K` (signo Kelvin).
     - `parse_version` solo con dígitos ASCII y componentes que caben en `u64`; el
       `protocol` de Quake 3 en `identify` sin `_` ni dígitos Unicode.
     - La descarga del instalador tiene un plazo total de 30 min para el cuerpo (Python:
       10 s por lectura, sin total).
     - RCON corta cada respuesta en 1 MiB y 1024 datagramas (Python no tiene tope).
     - El zip portátil se extrae con un bucle propio que nunca crea enlaces simbólicos y
       corta cada archivo en su tamaño declarado; se rechaza lo mismo que en Python.
     - Para la fase 6: el script `.cmd` del portátil copia primero `_internal`. Si el zip de
       2.0 no trae esa carpeta, `robocopy` sale con 16 y el script restaura la versión
       vieja: el empaquetado debe traer `_internal` o hay que cambiar el script.
   - **4c: mapas y nombres de países (`geo.country_name`). Hecha (2026-11-17); con ella,
     la fase 4 está completa.** Módulos de `jase-core`: `maps::{cod, levelshots, unreal,
     iwi, service}` y `geo::{country_names, country_name}`. Los nombres de países salen de
     `rust/jase-core/locales/countries.json` (lo escribe `scripts/export_locales.py`) y se
     comparan con el golden `rust/jase-core/tests/golden/countries.json`
     (`scripts/core_golden.py`), que corre sin `.venv`. Decodificar y reescalar imágenes va
     detrás de la *feature* `map-images` de `jase-core`, apagada por defecto (`jase-cli` no
     la compila): crate `image` 0.25 solo con JPEG, PNG y TGA, que suma 10 crates (`image`,
     `png`, `fdeflate`, `miniz_oxide` 0.8, `zune-jpeg`, `zune-core`, `moxcms`, `pxfm`,
     `bytemuck`, `byteorder-lite`) porque escribir JPEG/PNG y el Lanczos a mano no compensa;
     DXT1/3/5 a mano (`image` ya no lo trae), verificado contra Pillow.
     `MapImageService` recibe la descarga y el extractor local (`with_sources`) y es
     `Send + Sync`. Para la fase 5: activar `map-images` en el crate de la app y llamar a
     `get` con `spawn_blocking`. Divergencias deliberadas:
     - `archives_in` y las carpetas de `mods/` no siguen enlaces simbólicos (Python sí).
     - Un `.unr` de más de 128 MiB no se lee (Python usa `mmap` sin tope); el lector guarda
       índices de nombre, no copias. De sus `LevelInfo` solo se miran los 8 primeros
       (Python, todos): uno armado con millones al mismo cuerpo colgaría el worker.
     - Imágenes de afuera (levelshots y GameTracker) solo en JPEG, PNG o TGA (Pillow abre
       además BMP, GIF o WebP): sin firma conocida se lee como TGA y otra firma da `None`.
       Topes de 8192 px por lado y 256 MiB al decodificar.
     - Los tamaños de las miniaturas son los de Pillow (`Image.thumbnail`); los píxeles no
       (Lanczos de `image`, sin `reducing_gap`).
     - `casefold` de `pystr` difiere de Python en la sigma final y en ligaduras: cambia la
       clave de caché o la URL de GameTracker de algunos nombres no ASCII, nunca la
       seguridad de la ruta.
     - Un nombre de mapa que da un archivo de más de 255 bytes queda sin imagen (no se
       puede escribir en la caché; Python igual).
5. **App Iced:** ventana, tabla virtualizada, árbol de juegos, filtros, temas, i18n,
   diálogos, bandeja, instancia única, `jase://`, imágenes de mapas. **Hecha (2026-11-19)**,
   en subfases de la 5a a la 5j (ver [Paridad de la app](#paridad-de-la-app)):
   - **5a (base): hecha (2026-11-18).** Crate `rust/jase` (lib + binario `jase`) con Iced
     0.14 sin sus features por defecto: `wgpu`, `tiny-skia` (respaldo sin GPU,
     `ICED_BACKEND=tiny-skia`), `x11`, `wayland`, `tokio`, `crisp` y
     `linux-theme-detection`; `cargo tree -p jase -e normal` da 263 crates en Linux
     (`jase-core` solo, 63). Ventana con la lista de juegos, la tabla de servidores
     virtualizada (solo arma las filas visibles; orden por clic, selección con el teclado y
     colores `^`) y el escaneo con generaciones: un resultado de un escaneo viejo se
     descarta y solo uno terminado con algún master que respondió reemplaza la lista
     guardada de la sesión. Paleta de los 14 temas generada desde `theme.py`
     (`scripts/gen_palette_rs.py`). Al cerrar se guarda la config y el proceso sale con
     `std::process::exit`, sin esperar a las tareas bloqueantes. En WSL se fuerza X11
     (borra `WAYLAND_DISPLAY`). `jase --smoke` abre la ventana con 5.000 servidores falsos
     y sale por el mismo cierre con una tarea colgada (en el CI de Linux, con `xvfb-run`).
     El primer clic en otra columna ordena ascendente y en la misma invierte, igual que
     Qt 6 en 1.3.0 (comprobado con `QTest.mouseClick` en PySide6 6.12). Wayland nativo en
     KDE Plasma: pendiente de la prueba del dueño ([lista](pruebas-dueno-rust.md)); si falla, la 5b
     agrega el respaldo X11.
   - **5b (temas y gráficos): hecha (2026-11-18).** Feature `svg` de Iced (con `resvg` 0.45
     directo para el icono de la ventana); `cargo tree -p jase -e normal` da 294 crates en
     Linux. Los estilos de época (98, Luna, VGUI, Terminal) dibujan los bordes que Iced no
     tiene (un color por lado) con `theme/edges.rs`, franjas sobre el contenido en un `stack`
     (las esquinas no van en diagonal). Fuente y tamaño por estilo: Tahoma 8 pt en los de
     época, monoespaciada de 12 px en Terminal, 13 px en los planos. Iconos, banderas, flechas
     e insignias en SVG embebidos con `include_bytes!` desde `src/jase/assets/` (nada de disco
     en `view`). La flecha del orden sigue a la captura de 1.3.0: ascendente ▲. Barra de
     ocupación (plana) o por bloques (época), banderas en *País* y el modo claro/oscuro del
     sistema en vivo. `jase --screenshot <carpeta>` saca los 14 temas para compararlos con
     `scripts/screenshot.py`; las divergencias que quedan están en la fila de
     [Temas](#paridad-de-la-app).
   - **5c (barra lateral y filtros): hecha (2026-11-18).** Sin crates nuevos en el árbol
     (`indexmap` directo, ya lo traía `jase-core`; `iced_runtime` solo en los tests). El
     filtro vive en el modelo de la tabla (`ServerFilter`: texto, versión, filtros rápidos y
     marcas del árbol; recalcula el orden visible sin E/S y suelta la selección oculta). El
     país va por fila en memoria (base DB-IP cargada en un hilo al arrancar; su error, a la
     barra de estado). Árbol de juegos con carpetas Mods, Mapas y Países, marcas por juego
     que sobreviven a los refrescos y ramas en SVG (flechas en los planos, puntos y cuadro
     ± en 98/XP). Controles propios en los 5 estilos (`widgets.rs`: casilla, combo, campo,
     tooltip, modal) y un modelo de foco (`Focus`: árbol, tabla, campo, diálogo) que reparte
     el teclado. Filtros guardados con un modal propio y listas guardadas entre sesiones
     (lectura en `spawn_blocking` con número de pedido; escritura en un hilo propio, en
     orden, que sobrevive a un pánico; el cierre lo espera hasta 2 s, más hasta 2 s del historial). Comparada con las
     capturas de 1.3.0: alto de fila del árbol por tipo de fila, márgenes de Qt, banderas de
     16×12, combos de 16 px de línea y barra de estado de 22 px. Divergencias: el botón
     *Filtros guardados* es un `pick_list` (ancho de la opción más larga, menú plano) hasta
     la 5d; la lista del combo *País* sin banderas; nombres largos cortados sin «…» al medio.
   - **5d (menús, atajos y ventana): hecha (2026-11-18).** Sin crates nuevos (`iced_tiny_skia`
     solo en los tests, ya en el árbol). Menú propio con `overlay()` (`menu.rs`: barra de
     menús, botón de tema, *Filtros guardados* y contextual; se abre debajo del ancla, a su
     derecha o en el cursor y siempre cabe en la ventana; pasar por otro título o por un
     submenú lo abre; Esc, clic afuera o perder el foco lo cierran; casillas, radios, iconos y
     separadores de los 5 estilos). Las acciones son datos (`actions.rs`: las 31 de
     `_build_actions` más *Grupo*); lo de subfases futuras sale apagado en menús, barra,
     contextual y atajos. Barra de herramientas con el campo de texto y el combo de versión,
     atajos por foco (`F5`, `Ctrl+L`, `Ctrl+F`, `Ctrl+C` solo con la tabla, salir; nada con
     un diálogo, un menú o la lista de un combo abiertos) y barra de estado con progreso y
     mensajes con plazo. El escaneo vive en `ScanState` (generación, búsqueda en la red local
     que no reemplaza la lista de internet, refrescar un servidor con número de pedido).
     Columnas redimensionables y ancho de la barra lateral; tamaño, posición, columnas y orden
     se guardan en claves `rs_` de `ui_state` sin tocar los blobs de Qt. Comparada con las
     capturas de 1.3.0: alto de la barra de menús y de estado, títulos, encabezado, ítems de
     menú y márgenes de celda medidos de Qt.
   - **5e (ficha del servidor): hecha (2026-11-18).** Crates nuevos: `kamadak-exif` y
     `mutate_once` (los trae `image-without-codecs` de Iced; sin códecs: el PNG lo decodifica
     `jase-core` con `map-images`), `libc` (fuera de Windows) y más funciones de
     `windows-sys`. La ficha se arma en `view` desde el servidor elegido (`card.rs`) y un
     gancho `sync_card` tras cada `update` pide la imagen del mapa y recalcula el gráfico: la
     imagen va en `spawn_blocking` con 2 permisos (como los 2 hilos de Python) y número de
     pedido, y lo que espera permiso se aborta al cambiar de servidor. Jugadores y reglas en
     tablas propias (`details.rs`, orden de jugadores como Qt); las dos divisiones son
     `pane_grid` con su proporción en `rs_right_splitter`/`rs_details_splitter`. El historial
     de ocupación se lee en un hilo al arrancar y no se registra ni se guarda antes de
     leerse; se guarda cada 10 min y al cerrar, en orden (un guardado tardío no pisa al
     del cierre). Gráfico de barras sin `canvas` (`chart.rs`), hora local propia
     (`localtime.rs`) e icono del exe en el árbol solo en Windows (`exe_icon.rs`).
     Comparada con las capturas de 1.3.0: división tabla/ficha, márgenes, barra vertical que
     ocupa su lugar solo si hace falta, títulos en negrita solo con una fila elegida,
     botones de Luna y barra de estado de 98/VGUI medidos de Qt.
   - **5f (conectar y favoritos): hecha (2026-11-18).** Llavero con `keyring-core` y un
     store por plataforma (`zbus-secret-service-keyring-store` con `rt-async-io-crypto-rust`,
     `windows-native-keyring-store`) en vez de `keyring` 3: 28 paquetes contra 50, reusa
     `zbus` 5 y `windows-sys` 0.61 (`keyring` 3 traía un segundo `zbus`) y no deja `libdbus`
     ni OpenSSL en el `Cargo.lock`. Compatible con 1.3.0: en Windows lee y borra como
     `WinVaultKeyring` y escribe en el destino compuesto (`usuario@JimmysAllSeeingEye`); en
     Linux busca por `username`/`service` y escribe también el `application` y la etiqueta de
     Python, así 1.3.0 reemplaza en vez de duplicar. La clave de Steam se lee en un hilo al
     arrancar (el escaneo de Steam espera la respuesta). Conectar es una `Task` que guarda la
     contraseña y lanza el juego en un hilo (pedido numerado para la contraseña guardada);
     doble clic y Enter conectan. Vistas Favoritos y Recientes con número de pedido y 64
     consultas a la vez, filtros rápidos y de versión apagados mientras se ven; estrella,
     notas, grupos y recientes con sus menús. Comparada con las capturas de 1.3.0: alto de la
     lista lateral, orden de los botones del diálogo de contraseña, color de casillas y combos
     apagados y margen del nombre de un favorito sin respuesta.
   - **5g (agregar servidor, enlaces e instancia única): hecha (2026-11-18).** Crates
     nuevos: `rfd` 0.17.2 sin GTK (el portal de escritorio por `dlopen` de libdbus y
     `zenity` de respaldo; sin ninguno, como en WSL, el selector no abre) y `pollster`: 2
     paquetes. *Agregar servidor* (`add_server.rs`) sigue a `add_server.py` sin `regex` y
     mira solo los primeros 64 KiB del texto pegado; *Detectar* y la consulta de lo
     agregado son `Task` con número de pedido (una tardía se descarta). Exportar va en dos
     idas para escribir la config del momento en que llega la ruta. Instancia única sin
     crates: socket Unix en la carpeta temporal (`$TMPDIR` o `/tmp`, `jase-link-<usuario>`)
     y *pipe* `\\.\pipe\jase-link-<usuario>` en Windows, los dos compartidos con 1.3.0 (una
     abierta recibe el enlace de la otra); en Linux el socket solo se usa si es del usuario.
     Los enlaces no esperan a que se cierre el selector de archivos (no es modal). Comparada
     con las capturas de 1.3.0: columna de etiquetas del formulario solo con las filas
     visibles, espaciado de 6 px, alto de la lista de direcciones (192 px, el `sizeHint` de
     Qt) y alto y ancho mínimo de los botones de los diálogos (28 px en los planos; 60 px de
     texto como mínimo en 98, VGUI y Luna). Windows nativo (2026-11-18, MSVC): la release
     y los binarios de test compilan sin warnings, incluida la rama de Windows de
     `single_instance.rs`; la suite corrió una vez antes de la 5g (1275 bien; los 3 que
     fallaban se arreglaron en `8103580`); el dueño probó iconos del exe, estado de la
     ventana y la lectura de una contraseña guardada en el llavero de Windows.
   - **5h (ajustes y tareas de fondo): hecha (2026-11-19).** Sin crates nuevos (`ring` y `zip` solo como dev-dependencies, ya en el árbol por jase-core). Ajustes con
     pestañas propias (`widgets::tabs`: Iced 0.14 no trae; sin teclado) y sus diálogos
     internos (prompt y aviso de masters, *Buscar juegos*) dibujados sobre su tarjeta, sin
     pila de `Dialog`; el formulario y la pestaña Masters son datos puros (`settings::Form`,
     `masters_tab::MastersTab`). *Buscar juegos*, el estado de cada ruta y el ofrecimiento
     del primer arranque leen el disco solo en `Task`, con número de pedido. Masters diarios
     y versión nueva con una `FetchFn` inyectada (`should_check` también en el hilo);
     descarga con progreso acotado y una cancelación que no espera al hilo;
     `launch::spawn_detached` con `CREATE_NO_WINDOW` (lo que hace `QProcess::startDetached`);
     `update_version()` = `dev` mientras el crate sea 0.x (si no, ofrecería la 1.3.0 de
     Python). Refresco automático con `time::every`. Comparada con las capturas de 1.3.0:
     columna de nombres de la pestaña Juegos (216 px), páginas del mismo alto, filas de 30 px
     en la tabla de masters, columnas alineadas en *Buscar juegos* y el enlace de la barra
     legible en Terminal; la casilla de un master oficial elige su fila, como Qt.
   - **5i (vigilancias, herramientas y bandeja): hecha (2026-11-19).** La app pasa de
     `iced::application` a `iced::daemon`: *Buscar jugador*, *Ahora se juega* y cada consola
     RCON son ventanas aparte como en Qt (siguen abiertas mientras se usa la principal; un
     panel dentro de la ventana pediría arrastre, capas y pestañas propios), y las
     suscripciones de cierre, tamaño, clics y teclas se filtran por `window::Id` (la
     principal; Esc cierra una herramienta). Lo que en Qt es modal (*Amigos*, el umbral de
     partida, «Hay sitio») sigue como `Dialog` dibujado. Vigilar sitio (cada 10 s, hasta 30
     min), el umbral de partida y la revisión de fondo de amigos, favoritos y recientes
     (cada 2 min) corren en `Task` abortables con número de vuelta; *Ahora se juega*
     escanea de a 3 juegos con su número de pedido. La clave RCON se lee y se guarda en el
     llavero en un hilo y viaja como `Secret` (nunca en la barra, errores, `Debug` ni
     config). Bandeja: `ksni` 0.3.7 (`async-io`, el mismo `zbus` del llavero) + globos por
     `org.freedesktop.Notifications` con `zbus` en Linux, y `Shell_NotifyIconW` a mano en
     Windows (un hilo con ventana oculta y su bucle de mensajes); `tray-icon` arrastraba
     GTK 3 y 51 paquetes al `Cargo.lock` y no da globos, `notify-rust` sumaba `winrt` y un
     StatusNotifierItem propio eran ~600 líneas. Suma 5 paquetes, solo en Linux
     (`arc-swap`, `crossbeam-queue`, `ksni`, `pastey`, `task-local`). Sin bandeja (WSL, sin
     D-Bus) los avisos van a la barra de estado y minimizar no oculta; en Wayland tampoco
     oculta (`is_minimized` no responde). `--screenshot` captura cada herramienta en su
     ventana sin un rearmado de la vista en medio (un `text_editor` rearmado sale vacío en
     la captura). Comparada con las capturas de 1.3.0: *Quitar* de *Amigos* a todo el
     ancho y el número del umbral elegido al abrir, como `QSpinBox`.
   - **5j (paridad y cierre): hecha (2026-11-19).** Sin crates nuevos. Recorrido a mano de
     cada punto de la guía y de cada fila de la tabla (WSL con XTest, tests o prueba del
     dueño); lo roto se arregló (tooltip de un combo encima de su lista, *Editar nota…* sin
     el texto elegido, *Acerca de* sin icono). `rust/jase/tests/parity_docs.rs` ata la
     guía a la tabla. `--screenshot` suma el menú contextual, *Acerca de*, *Editar nota…* y
     «Hay sitio»: 238 capturas por lado (17 vistas × 14 temas). `rust/jase/tests/perf.rs`
     mide con 1.000 y 5.000 filas por proporción y por tope (sin plazos finos en el CI);
     en release, con 5.000: refresco automático 58 ms (75 ms antes), filtro de texto 9 ms,
     ordenar por cada columna 3,5 ms, *Buscar jugador* en 4 listas de 5.000 136 ms y
     3.850 B por fila. Actualizar una fila ya no recorre el orden entero
     (`partition_point`) y la poda del refresco automático es una sola pasada
     (`TableModel::retain`). *Buscar jugador* y *Ahora se juega* se desplazan hasta la
     fila elegida con ↑/↓. Accesibilidad anotada en
     [Divergencias de la fase 5](#divergencias-de-la-fase-5). Lo que solo puede probar o
     decidir el dueño: [pruebas-dueno-rust.md](pruebas-dueno-rust.md). La rama `rust` se
     subió el 2026-11-19: el CI de `a63e29f` quedó verde en Linux y Windows (en Windows,
     2361 tests bien y 1 ignorado, el llavero real).
   - Sigue: fase 6, empaquetado.
6. **Empaquetado:** binario de Windows (instalador + zip portátil) y de Linux (AppImage o
   binario), CI con `cargo test`, `clippy` y `fmt`. Mismos nombres de archivo en el
   Release (`JASE-<ver>-setup.exe`, `JASE-<ver>-win64.zip`) y mismo AppId de Inno Setup,
   para que el actualizador de 1.3.0 instale la 2.0; el instalador y el zip borran los
   archivos de PyInstaller que ya no se usan.
7. **Cambio:** cuando haya paridad, la versión Rust se publica como **2.0.0**, se borra
   el código Python y la documentación pasa a describir Rust.

## Paridad de la app

Una fila por punto de la [guía de uso](../guia/guia-usuario.md); la 5j la recorre entera con
capturas de 1.3.0 al lado. `rust/jase/tests/parity_docs.rs` exige una fila por título (desde
*Descarga*) y por viñeta de *Uso*, y que ningún «pendiente» quede sin dueño (del dueño, fase 6
o fase 7).

| Punto de la guía | Subfase | Estado |
|---|---|---|
| Descarga | fase 6 | pendiente de la fase 6: instalador y zip portátil del binario de Rust (la config sigue en `%APPDATA%\JimmysASE`) |
| Carpetas de los juegos (primer arranque, Ajustes) | 5h | hecho: rutas con su estado y *Examinar…* en Ajustes, y el ofrecimiento de *Buscar juegos* del primer arranque |
| Elegir el juego y refrescar la lista | 5a, 5d | hecho: elegir el juego escanea (5a); *Refrescar lista*, *Detener*, *Refrescar servidor* y `F5` (5d) |
| Doble clic en un servidor para entrar | 5f | hecho: doble clic y Enter (con contraseña del llavero) |
| Filtros rápidos | 5c | hecho: ocultar vacíos, llenos, con contraseña y solo bots, ping máximo y país; texto y versión en la barra de herramientas (5d) |
| Columna *Jugadores* (personas y bots) | 5a, 5b | hecho: «n/max», tooltip con bots y barra de ocupación (plana o por bloques) |
| Atajos | 5d, 5f, 5g, 5i | hecho: flechas, Re Pág/Av Pág e Inicio/Fin (5a); `F5`, `Ctrl+L`, `Ctrl+F`, `Ctrl+C` y salir (5d); Enter conecta (5f); `Ctrl+N` y `Ctrl+V` con la tabla enfocada (5g); `Ctrl+J`, `Ctrl+Shift+F`, `Ctrl+Shift+R` y `Ctrl+Shift+A` (5i) |
| Menú contextual | 5d, 5f, 5g, 5i | hecho: clic derecho en una fila (la elige) o en el vacío; *Copiar IP:puerto*, *Refrescar servidor*; conectar, favoritos, notas, grupos y quitar de recientes (5f); *Copiar enlace jase://* y *Agregar servidor…* (5g); las vigilancias (5i) |
| Ficha del servidor | 5e, 5f | hecho: ficha, imagen del mapa, jugadores (ordenables) y reglas, *Copiar*, divisiones guardadas (5e), estrella y nota (5f) |
| Favoritos | 5f, 5g | hecho, con el clic derecho en ★ de la lista lateral (5g) |
| Agregar servidor | 5g | hecho (el foco inicial va al campo IP, no al combo) |
| Recientes | 5f | hecho |
| Grupos de favoritos | 5f, 5g | hecho |
| Notas de favoritos | 5f | hecho |
| Exportar / importar favoritos | 5g | hecho (el selector de archivos abre sin ventana padre) |
| Filtros guardados | 5c, 5d | hecho: guardar, aplicar y borrar combinaciones desde su menú, con separador y submenú *Borrar* |
| Avisar cuando haya sitio | 5i | hecho: cada 10 s hasta 30 min, «Vigilando…» en la barra y «Hay sitio» con *Conectar* (con otro diálogo abierto, a la barra de estado) |
| Avisar cuando se arme partida | 5i | hecho: umbral por servidor, aviso una vez al cruzarlo y de nuevo tras bajar a la mitad |
| Árbol de juegos y barra de herramientas | 5c, 5d, 5e | hecho: árbol (5c), barra de herramientas con sus grupos, campo y combo (5d) e icono del exe en el árbol, solo en Windows (5e) |
| Red local (LAN) | 5d | hecho: *Buscar en la red local* y `Ctrl+L`; la lista LAN no reemplaza la de internet |
| Ahora se juega | 5i | hecho: ventana aparte; juegos listos de a 3 escaneos, la lista de la sesión y doble clic que revela el servidor |
| Consola RCON | 5i | hecho: una ventana por servidor, clave en el llavero, historial con ↑/↓ |
| Amigos | 5i | hecho: lista, aviso al aparecer (en tandas), negrita en los jugadores, clic derecho *Agregar a amigos* y revisión de fondo cada 2 min |
| Bandeja del sistema | 5i | hecho: icono, menú, globos con clic y minimizar a la bandeja (en Wayland y sin bandeja, como en WSL, no se oculta ni hay globos); pendiente la prueba del dueño en KDE y Windows ([lista](pruebas-dueno-rust.md)) |
| Enlaces jase:// | 5g | hecho: abrir, confirmar e instancia única; registrar el esquema para el binario de Rust, fase 6 |
| Historial de ocupación | 5e, 5f | hecho: registro de favoritos y recientes, gráfico por hora local en la ficha |
| Ventana (tamaño, divisiones, columnas, último juego) | 5d, 5e | hecho: tamaño, posición (salvo en Wayland), maximizada, barra lateral, anchos y orden de columnas y último juego (5d); las divisiones de la ficha y los jugadores (5e) |
| Temas (Ver → Tema) y aspecto | 5b, 5d | hecho para ventana, lista, tabla, barra de estado y tooltip (5a, 5b), salvo divergencias aceptadas: barras de desplazamiento sin flechas, trama ni estrías; línea discontinua de Terminal continua; esquinas de los biseles rectas; sin barra visible, la selección de época no llega al borde. Pendiente del dueño (decisión): la insignia apagada del juego elegido y el radio de los menús planos. La 5c suma el árbol, la casilla, el combo, el campo de texto, el tooltip y el modal en los 5 estilos; la 5d, Ver → Tema (cambia en caliente), los menús, la barra de herramientas y la de estado (sin navegación con flechas en los menús). Cada widget nuevo se estiliza en su subfase |
| Buscar juegos | 5h | hecho: desde el menú y desde Ajustes, con *Buscar en carpeta…* y *Cancelar búsqueda* |
| Masters | 5a, 5h | hecho: el escaneo usa los oficiales y los personales (5a); la pestaña de Ajustes con agregar, quitar, probar y restaurar, y la lista oficial revisada cada 24 h (5h) |
| Refresco automático | 5h | hecho: cada 2–15 min sin vaciar la lista ni pisar un escaneo, un diálogo o la ventana minimizada |
| Actualizar desde la app | 5h | hecho: aviso en la barra de estado y *Buscar actualizaciones…*; la instalación en Windows, solo con tests y compilada (la fase 6 empaqueta `JASE.exe`) |
| Idioma | 5a, 5h | hecho: sigue el del sistema o el de la config al arrancar (5a); la elección en Ajustes con aviso de reinicio (5h) |
| Juegos de Steam y la clave de la API web | 5f, 5h | hecho: la clave sale del llavero (5f; sin clave se avisa en la barra de estado) y se edita y guarda en Ajustes (5h) |
| Imágenes de mapas | 5e, 5h | hecho: de la carpeta del juego o descargadas, con caché junto a la config; se vuelve a pedir al aceptar Ajustes (5h) |
| País de los servidores | 5c, 5h | hecho: bandera, orden y tooltip en *País*, carpeta *Países* del árbol y combo *País* (su lista sin banderas); el botón de Ajustes que baja la base de nuevo (5h) |
| Configuración y contraseñas | 5a, 5f | hecho: se lee y se guarda la config (5a); contraseñas en el llavero, compatibles con las de 1.3.0 (5f) |
| Línea de comandos sin interfaz | 3 | hecho (fase 3, `jase-cli`) |
| Acerca de y créditos | 5d | hecho: *Acerca de* con el icono de la app, nombre y versión, como 1.3.0 (la del crate hasta la fase 7); los créditos de la guía (Iced en vez de PySide6) y la licencia de las banderas en el paquete, pendientes de la fase 6 |
| Accesibilidad | 5j | divergencia aceptada: sin lector de pantalla hasta que Iced integre AccessKit (Iced 0.14 no lo trae); atajos, foco propio, escala por DPI y temas de alto contraste sí; Tab entre campos, decisión del dueño ([lista](pruebas-dueno-rust.md)) |

## Calidad

- `cargo test`, `cargo clippy -- -D warnings` y `cargo fmt --check` verdes en cada
  commit de la rama. La suite de Python sigue verde mientras exista.
- Los tests de Python se portan junto con el código de cada fase: son la especificación.
- `scripts/check_docs.py` sigue corriendo; las docs se actualizan en el mismo commit que
  las cambia.

## Divergencias de la fase 5

- Iced no trae barra de menús ni diálogos de archivo nativos: los menús se dibujan
  (`menu.rs`) y los diálogos de archivo usan los del sistema (`rfd`).
- Bandeja del sistema (hecha en la 5i: `ksni` en Linux, `Shell_NotifyIconW` en Windows),
  redimensionar y reordenar columnas (5d): construidos o integrados en la fase 5.
- Accesibilidad: se anota, no se construye. Iced 0.14 no integra AccessKit (`accesskit` no
  está en el `Cargo.lock`; `iced_winit` 0.14.1 no conecta el de `winit` 0.30) ni da texto
  alternativo a `svg`/`image`: Narrador, NVDA y Orca solo ven la ventana y su título. Sí hay
  atajos, foco propio (árbol, tabla, campo, diálogo), escala por DPI del sistema, temas de
  alto contraste (Terminal, Clásico; contraste comprobado en `rust/jase/tests/theme.rs`) y
  tooltips. `operation::focus_next`/`focus_previous` existen sin conectar: Tab no recorre
  campos (decisión del dueño). Se revisa en cada versión de Iced ([deuda](deuda.md)).
- Lo que solo puede probar o decidir el dueño (KDE, Windows real, divergencias por
  aceptar o arreglar; el CI de Windows ya está verde): [pruebas-dueno-rust.md](pruebas-dueno-rust.md).

## Para quien lo construye

- Elegir los crates (tokio, serde, keyring, image/resvg, tray, instancia única, HTTP)
  priorizando Rust puro para que la compilación cruzada no necesite C++.
- El formato de los catálogos de traducción en Rust.
- El detalle de cómo se reparten los mixins `window_*.py` en módulos de Iced.
- Cómo forzar X11 en WSLg (por ejemplo, detectar WSL al arrancar).

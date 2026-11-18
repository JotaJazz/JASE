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
     errores de `recv_from` y sigue leyendo (lo prueba el CI de Windows, no una máquina real). Divergencia
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
     `flate2`; escribir un lector de zip propio no compensa). Lo de disco y procesos es
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
   - **4c: mapas y nombres de países (`geo.country_name`).**
5. **App Iced:** ventana, tabla virtualizada, árbol de juegos, filtros, temas, i18n,
   diálogos, bandeja, instancia única, `jase://`, imágenes de mapas.
6. **Empaquetado:** binario de Windows (instalador + zip portátil) y de Linux (AppImage o
   binario), CI con `cargo test`, `clippy` y `fmt`. Mismos nombres de archivo en el
   Release (`JASE-<ver>-setup.exe`, `JASE-<ver>-win64.zip`) y mismo AppId de Inno Setup,
   para que el actualizador de 1.3.0 instale la 2.0; el instalador y el zip borran los
   archivos de PyInstaller que ya no se usan.
7. **Cambio:** cuando haya paridad, la versión Rust se publica como **2.0.0**, se borra
   el código Python y la documentación pasa a describir Rust.

## Calidad

- `cargo test`, `cargo clippy -- -D warnings` y `cargo fmt --check` verdes en cada
  commit de la rama. La suite de Python sigue verde mientras exista.
- Los tests de Python se portan junto con el código de cada fase: son la especificación.
- `scripts/check_docs.py` sigue corriendo; las docs se actualizan en el mismo commit que
  las cambia.

## Riesgos conocidos

- Iced no trae barra de menús, diálogos de archivo nativos ni lectores de pantalla. Los
  menús se dibujan (widget propio o `iced_aw`), los diálogos de archivo usan los nativos
  del sistema (`rfd`) y la accesibilidad queda como diferencia conocida frente a Qt.
- Bandeja del sistema, redimensionar y reordenar columnas: hay que construirlos o
  integrarlos; se estiman al inicio de la fase 5.

## Para quien lo construye

- Elegir los crates (tokio, serde, keyring, image/resvg, tray, instancia única, HTTP)
  priorizando Rust puro para que la compilación cruzada no necesite C++.
- El formato de los catálogos de traducción en Rust.
- El detalle de cómo se reparten los mixins `window_*.py` en módulos de Iced.
- Cómo forzar X11 en WSLg (por ejemplo, detectar WSL al arrancar).

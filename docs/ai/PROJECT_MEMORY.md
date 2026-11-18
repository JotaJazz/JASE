# Memoria del proyecto

Solo se agrega arriba; nunca se reescribe lo anterior. Cada etapa: 3 viñetas como mucho
(plantilla en [DEVELOPMENT_GUIDE](DEVELOPMENT_GUIDE.md)). No es un changelog: el detalle
está en git y en `packaging/release-notes.md`.

## 2026-11-18 — port a Rust, revisión final de la 5b
- Tests de Rust fiables bajo carga: un solo candado del idioma por crate (`rust/jase-core/tests/support/lang.rs` y `rust/jase/tests/support/lang.rs`), plazos en vez de cuentas y 180 s para las pruebas con ventana; el job de Rust del CI tiene 60 min y xvfb a 1920×1080.
- `--screenshot` ignora el modo del sistema; los títulos del encabezado que no caben se cortan con «…» midiendo con `Paragraph` (feature `advanced` de Iced, sin crates nuevos).
- Abierto: caché de cargo en el CI si el job se acerca al límite ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-18 — port a Rust, fase 5b (temas y gráficos)
- La app Iced tiene los estilos de época (98, Luna, VGUI, Terminal) con bordes por lado (`theme/edges.rs`), fuentes por estilo, iconos, banderas e insignias SVG embebidos, barra de ocupación y modo del sistema en vivo; `jase --screenshot` saca los 14 temas y se corrigió lo que difería de las capturas de 1.3.0 (flecha ▲ ascendente, filas de 28 px, encabezado centrado, selección sin foco).
- Descartado: un widget propio con `advanced` para los biseles y leer los SVG de disco en `view` ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la 5c (barra lateral y filtros), las divergencias que decide el dueño y la prueba de Wayland ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-18 — port a Rust, fase 5a (base de la app)
- `rust/jase` (Iced 0.14) abre la ventana con la lista de juegos, la tabla de servidores virtualizada (orden, selección, colores `^`) y el escaneo con generaciones y lista de la sesión; se prueba sin pantalla con tests sobre `App::update` y la tabla, y con `jase --smoke` (5.000 servidores falsos, `tiny-skia`, `xvfb-run` en el CI de Linux).
- Descartado: `iced::widget::table` (arma todas las filas), `iced_aw` y esperar a las tareas al cerrar (sale con `process::exit`) ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la 5b (temas) y la prueba de Wayland nativo en KDE del dueño ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-17 — port a Rust, fase 4c (mapas y países)
- `jase-core` saca la imagen de un mapa de los archivos del juego (levelshots de `.pk3`, pantallas de carga IWI de CoD, capturas de `.unr`) o de GameTracker y la guarda en caché como PNG, y da los nombres de países en los 6 idiomas; tests de Python traducidos con extractores y descargas falsos, DXT y miniaturas contra Pillow; con esto la fase 4 está completa.
- Descartado: dejar las imágenes para la fase 5 (van en la *feature* `map-images`) y un crate para DXT ([divergencias](../hoja-de-ruta/port-rust.md)).
- Abierto: fase 5, la app Iced ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-17 — port a Rust, fase 4b (sistema)
- `jase-core` busca los exes y carpetas de los juegos (registro, Steam, carpetas), arma y lanza la línea de comandos sin shell, reconoce el juego de un servidor, busca en la LAN, habla RCON y avisa, descarga y aplica actualizaciones (instalador y zip portátil); tests de Python traducidos con falsos locales y comparados con el `.venv`, y lo de Windows (registro, `cmd.exe`/`robocopy`) solo en su CI.
- Descartado: `SIO_UDP_CONNRESET`, una shell o un crate para `shlex` y un crate o `reg.exe` para el registro ([divergencias](../hoja-de-ruta/port-rust.md)).
- Abierto: fase 4c (mapas, nombres de países) y, para la 6, el `_internal` que espera el script portátil ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-17 — port a Rust, fase 4a (datos)
- `jase-core` lee y escribe config, favoritos, notas, grupos, recientes, listas guardadas, historial y la exportación de favoritos en el formato de Python 1.3.0; se probó con goldens de Python (mismos bytes al reguardar) y Python relee la config que escribe Rust. Suma amigos, alertas de partida, búsqueda de jugadores y enlaces `jase://`.
- Descartado: imitar el descarte de claves desconocidas de la config; Rust las conserva para que 1.3 y 2.0 convivan ([divergencias](../hoja-de-ruta/port-rust.md)).
- Abierto: fase 4b (rutas, detección, launcher, `identify`, LAN, RCON, actualización) y 4c (mapas, nombres de países); `passwords.py` va en la 5.

## 2026-11-17 — port a Rust, fase 3 (`jase-cli`)
- `rust/jase-cli` da los mismos comandos, JSON (schema 1) y códigos de salida que la CLI Python 1.3.0, con la base de países DB-IP; un test corre los dos binarios contra los mismos falsos y no encontró diferencias.
- Descartado: imitar el `--help` y los mensajes genéricos de argparse; los de clap quedan como divergencia deliberada ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: esa paridad no corre en el CI, que no tiene `.venv` ([deuda](../hoja-de-ruta/deuda.md)); sigue la fase 4, resto del núcleo.

## 2026-11-17 — port a Rust, fase 2b (escáner)
- `jase-core` escanea completo: masters de las 6 familias, API web de Steam con su filtro de spam, consultas en paralelo por un socket compartido y la descarga diaria de `masters.json`; tests de Python traducidos con masters, servidores y HTTP falsos locales.
- Descartado: portar `SIO_UDP_CONNRESET` (el lector ignora los errores de `recv_from`) y una tarea por escaneo (`tokio::spawn`): todo es un future que se cancela al soltarlo ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: el ICMP de Windows solo lo prueba el CI ([deuda](../hoja-de-ruta/deuda.md)); sigue la fase 3, `jase-cli`.

## 2026-11-15 — port a Rust, fase 2a (juegos, i18n, masters)
- `jase-core` suma la tabla de juegos (generada desde `games.py` y verificada contra un volcado de Python), la traducción con los catálogos exportados a JSON y las listas de masters sin descarga.
- `masters.custom.json` escrito por Rust lo lee Python 1.3.0 (test con el `.venv`): se puede volver de la 2.0 sin perder masters propios.
- Pendiente para 2b: escáner, descarga de `masters.json` y API web de Steam; migración desde la config, en la fase 4.

## 2026-11-15 — port a Rust, fase 1 (protocolos)
- `rust/jase-core`: las 6 familias + Huffman + API web de Steam portadas desde `src/jase/protocols/`, con sus tests de Python traducidos ([diseño](../hoja-de-ruta/port-rust.md)).
- Paridad con Python 1.3.0 probada sobre cada fixture (`scripts/protocol_golden.py` → `rust/jase-core/tests/golden/`) y robustez con `proptest`; CI con un job de Rust.
- Tres divergencias deliberadas donde Python se cae con una excepción no prevista: Rust descarta el servidor o la entrada.

## 2026-11-05 — README para el público general
- README (×2) corto: qué es, juegos por familia, cómo empezar; el detalle pasa a `docs/guia/` (guía de uso y juegos y masters, inglés + español).
- Tabla de masters al día: faltaban dpmaster (Q3) y master.ouned.de (JK2).

## 2026-11-05 — 1.3.0
- Publicada con las listas guardadas entre sesiones, el auto-refresco en su sitio, los masters nuevos y las correcciones de las dos auditorías.
- Abierto: «mostrar» de la segunda instancia sin verificar en Windows (ver deuda).

## 2026-11-05 — sistema de documentación
- Nuevo `docs/` (arquitectura, ADR, desarrollo, deuda, glosario, capa de IA) y `AGENTS.md`; `CLAUDE.md` queda local y remite a `AGENTS.md`.
- `scripts/check_docs.py` corre en los tests: enlaces, rutas, datos personales y esta memoria.
- Los README siguen siendo para usuarios (inglés + español).

## 2026-11-05 — listas guardadas entre sesiones
- Al abrir, cada juego muestra su último escaneo completo sin escanear, como el ASE original (ADR 0006).
- Descartado: refrescar solo al abrir (la lista se movería mientras se elige) y dejar en gris los caídos (desaparecen al refrescar).
- Abierto: guardado perdido si se cierra justo tras un escaneo; ver deuda.

## 2026-11-05 — segunda auditoría y auditoría de la UI
- Corregidos: segunda instancia que pisaba la config, caché a medias, filtros en Favoritos, favoritos perdidos al cambiar de juego, disco en el hilo de la UI, auto-refresco que vaciaba la tabla.
- Masters nuevos con sondeo real: dpmaster.deathmask.net (Q3) y master.ouned.de (JK2); el resto de la lista propuesta estaba muerta.
- Abierto: juegos nuevos (Deus Ex, Unreal, Sauerbraten, AssaultCube, Red Eclipse, DDNet) con plan escrito y sin ejecutar.

## 2026-10-31 — 1.2.1, primera auditoría
- Correcciones de seguridad (texto remoto en texto plano, mods validados) y fiabilidad (config dañada a `.bad`, números absurdos acotados).
- Actualización portátil más segura: copia `_internal` antes que el exe; probada con cmd.exe.

## 2026-10-30 — 1.2.0, Tactical Ops
- Tactical Ops como juego aparte, separado de la lista de UT99 por el `gametype`.

## 2026-10-29 — 1.1.0
- La lista de masters empaquetada gana a una caché más vieja (ADR 0004): con la caché vieja, CS 1.6 fallaba.

## 2026-10-28 — jase-cli
- Consultas sin interfaz con JSON versionado; el núcleo queda sin Qt y la app pasa al extra `gui` (ADR 0001).

## 2026-10-27 — 1.0.0
- Primera versión estable; la recolección de basura pasa al hilo de la UI tras «access violation» en Windows (ADR 0002).
- Fuera a propósito: QuakeWorld (masters muertos) y Halo CE (enctypeX solo GPL).

## 2026-10-19 — masters.json
- Masters oficiales en `masters.json` descargable y personales en `masters.custom.json` (ADR 0004).

## 2026-10-18 — inicio
- Diseño v1, protocolos Quake3 (CoD) y GameSpy (UT99); regla de sondeo real para cada juego (ADR 0005).

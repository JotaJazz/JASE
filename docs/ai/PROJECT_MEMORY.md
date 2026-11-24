# Memoria del proyecto

Solo se agrega arriba; nunca se reescribe lo anterior. Cada etapa: 3 viñetas como mucho
(plantilla en [DEVELOPMENT_GUIDE](DEVELOPMENT_GUIDE.md)). No es un changelog: el detalle
está en git y en `packaging/release-notes.md`.

## 2026-11-23 — columnas Flux y PL (rama `dev`)
- `ServerInfo.lost` (intentos sin respuesta antes de la respuesta; en un sin conexión, `query_retries + 1`) lo llenan `exchange`, UT2004 y Valve; la tabla guarda por dirección las últimas 20 muestras (`Link`, sobrevive a `clear`): Flux = media de |Δping| entre respuestas al primer intento, PL = % perdido. La lista guardada entra con `add_cached` y no cuenta.
- Columnas Flux y PL tras Ping (`enum Column` pasa a 10): la cabecera guardada por la 2.1.0 (`8;…`) se descarta una vez y vuelven los anchos de fábrica. Redimensionar columnas ya existía (asa de 4 px).
- Descartado: varias sondas por escaneo (multiplica el tráfico y choca con el anti-flood de Zandronum). Abierto: sin probar contra servidores reales; flux y PL no se guardan entre sesiones.

## 2026-11-21 — README en seis idiomas con capturas nuevas
- README en inglés, español, portugués, francés, alemán e italiano (`README.<idioma>.md`, los revisa `check_docs.rs`), con secciones «Más que el viejo ASE» (enlaces `jase://`, pegar direcciones, avisos, Ahora se juega…) y «Trae tu juego o tu master» (issue, `masters.json` llega sin versión nueva, guía de agregar juego).
- Capturas en `docs/img/<idioma>/` (principal, menú contextual, Ahora se juega, Ajustes → Juegos) y `docs/img/themes.png`, con `JASE_SCREENSHOT_LANG` en el modo de capturas; se borran `docs/screenshot*.png`.
- Abierto: las guías siguen solo en inglés y español; los README nuevos enlazan a la inglesa.

## 2026-11-21 — versión 2.1.0
- `rust/Cargo.toml` pasa a 2.1.0 y `packaging/release-notes.md` describe los menús, la barra configurable, las pestañas Juegos/Avisos/Red y Ayuda; `dev` entra en `master` y se publica con la receta de [publicar](../desarrollo/publicar.md).
- La prueba `installed_2_0_0_sees_only_newer_releases` fija la versión instalada en 2.0.0 en vez de la del crate.
- Abierto: las capturas del README siguen con la barra vieja.

## 2026-11-21 — pestaña Juegos al estilo OpenASE (rama `dev`)
- Ajustes → Juegos: lista con insignias en *Instalados* (carpeta hallada o buscándose, o Steam) y *No instalados*, y la ficha del elegido con carpeta, *Examinar…* y estado; abre en el primer juego con carpeta. Mismos mensajes `Path`/`Browse`: la búsqueda del exe no cambia.
- Descartado: lo que OpenASE pone por juego (nombre de jugador, archivo de config, invitaciones): JASE no los usa al lanzar.
- Abierto: la lista no se maneja con el teclado; los juegos de Steam salen en *Instalados* aunque no estén en el disco (como «Solo juegos instalados»).

## 2026-11-21 — Ajustes → Red y proxy HTTP (rama `dev`)
- Pestaña Red: proxy `http(s)://` o `socks4/5://` para todo lo HTTP (un solo `net::agent_builder()`; `socks-proxy` de ureq, sin crates nuevos), validado antes de guardar, y espera, reintentos, consultas simultáneas y espera del master para el escaneo de listas (config solo si no son de fábrica). Ajustes pasa a 660 px por las 7 pestañas; «Avisos» se acorta en en/fr/de. Capturas: 280 (20 vistas).
- Descartado: proxy para UDP (SOCKS5 UDP casi no existe en proxies reales) y llevar los valores de red a `query_one` (cambiaría la interfaz que usan los falsos de los tests).
- Abierto: probar contra un proxy real ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-21 — Ajustes → Avisos (rama `dev`)
- Pestaña Avisos: casillas para el aviso de amigos, la revisión de fondo (la misma de *Amigos*), el aviso de partida y pedir atención a la barra de tareas, más la lista de umbrales de partida con *Quitar*; config `notify_friends`, `notify_party` y `notify_attention` (encendidas, se escriben solo apagadas para conservar los bytes de 1.3.0). Capturas: 266 (19 vistas).
- Descartado: sacar Vigilar sitio y el umbral del menú Servidor (son por servidor), sonido propio (sin biblioteca de audio) y cambiar el intervalo de 2 min.
- Abierto: los avisos de «Hay sitio» no tienen casilla (solo existen si el usuario vigila un servidor).

## 2026-11-21 — barra de herramientas configurable (rama `dev`)
- Pestaña Ajustes → Barra: casilla de visible, Subir/Bajar, separadores y Restablecer; la config guarda `toolbar` como `[[id, visible], …]` (vacío = de fábrica, no se escribe) y la app ignora ids desconocidos y agrega ocultos los botones que falten; Copiar vuelve como botón opcional. Las capturas pasan a 252 (18 vistas).
- Descartado: botones sin icono (Ahora se juega, RCON, LAN) en la barra y arrastrar para ordenar (Iced 0.14 no lo trae).
- Abierto: la lista de la pestaña no se maneja con el teclado.

## 2026-11-20 — menú Ayuda y créditos (rama `dev`)
- Ayuda: guía en GitHub por idioma (`F1`), Licencia (el `LICENSE` incrustado), Reportar un problema, Sitio web, actualizaciones y Acerca de con créditos (Iced, tokio, ureq, resvg, keyring, DB-IP, GameTracker, masters comunitarios, OpenASE) y «Licencias de terceros…», que abre `licenses/THIRD-PARTY.txt` junto al exe.
- Descartado: cambiar la licencia (el dueño sigue con MIT) y meter la lista de `cargo about` en el binario (ya viaja en los paquetes).
- Abierto: sin paquete (compilado a mano) «Licencias de terceros…» solo avisa en la barra de estado.

## 2026-11-20 — menús, barra e iconos al estilo OpenASE (rama `dev`)
- Ocho menús (Jugador, Amigos y Herramientas nuevos; Ajustes y RCON a Herramientas), barra Actualizar/Detener/Refrescar | Conectar/Favorito | Agregar | Buscar jugador/Amigos | Ajustes, iconos modernos teñidos con colores que ya tenía `Palette` (Terminal sigue monocromo) y Favoritos, Recientes y Amigos con icono arriba del árbol; 4 iconos nuevos en los 3 estilos.
- Descartado: copiar código o iconos de OpenASE (GPL-3; JASE es MIT: solo la disposición), una vista de servidores con amigos (Amigos abre su diálogo) y el botón de tema y Copiar en la barra (siguen en Ver → Tema y Servidor).
- Abierto: los accesos del árbol siguen sin teclado ([deuda](../hoja-de-ruta/deuda.md)); capturas del README sin rehacer.

## 2026-11-20 — 2.0.0 publicada (cierre del port a Rust)
- Python retirado del repo (queda en la tag `v1.3.0`), `release.yml` reusa `package.yml` y crea solo un borrador (con un freno que falla cerrado si la release ya está publicada); `master` recibió `rust` y la tag `v2.0.0`; el dueño probó el borrador (instalador, portátil y vuelta a 1.3.0) y la actualización desde la app de 1.3.0 antes de *latest*.
- Descartado: adoptar en el merge el bump de acciones de dependabot (#1) sin probarlo; publicar la prerelease antes de la prueba del dueño (1.3.0 ve las prereleases) ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la actualización Rust→Rust hasta una 2.0.1, KDE y el AppImage, y las divergencias aceptadas ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-19 — port a Rust, fase 7 (2.0.0 y retiro de Python)
- La versión pasa a 2.0.0 y `update_version()` compara de verdad; los tests de Rust ya no usan Python (respuestas congeladas como valores fijos; verificador de la documentación y contratos del empaquetado en Rust); la documentación describe la app de Rust (reglas, arquitectura, ADR 0001–0006, comandos, guía, CLI) y el dueño aprobó en su Windows la bandeja, el instalador y el exe.
- Descartado: meter `jase-cli` en los paquetes (como 1.3.0), portar los scripts de Python sin reemplazo (quedan en la tag `v1.3.0`) y dejar el ADR 0002 como norma (no aplica en Rust) ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: borrar Python, `release.yml` de Rust y la publicación (borrador → prerelease → *latest*); la actualización Rust→Rust hasta una 2.0.1 ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-19 — port a Rust, fase 6 (empaquetado)
- Fase 6 hecha: `package.yml` arma como *artifacts* el instalador, el zip portátil (con `_internal\README.txt` para el script de 1.3.0) y el AppImage (en `ubuntu-22.04`, glibc 2.35; ventanas con `application_id` `JASE` para el `WM_CLASS`) del binario de Rust, con los nombres de 1.3.0, avisos de terceros de `cargo about` y prueba de humo; el CI prueba el instalador y el portátil sobre la 1.3.0 publicada; el exe de Windows lleva icono, VersionInfo, subsistema de ventana (sin consola), CRT estático y `strip`. CI verde en `dec22c4`; el benchmark de `add_or_update` pasó a manual.
- Descartado: un binario suelto en Linux, cambiar `jase.iss`, `jase.desktop` o el script portátil de 1.3.0, y adaptar `release.yml` ahora (los fijan tests de Python; fase 7) ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la prueba del dueño con los paquetes ([lista](../hoja-de-ruta/pruebas-dueno-rust.md)) y la fase 7: `release.yml`, la 2.0.0, los créditos de la guía con Iced y borrar Python ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-19 — port a Rust, fase 5j (paridad y cierre de la fase 5)
- Fase 5 hecha: recorrido a mano de la guía entera (arreglados el tooltip encima de la lista de un combo, la nota sin el texto elegido y *Acerca de* sin icono), `parity_docs.rs` ata la guía a la tabla de paridad, 238 capturas por lado, *Buscar jugador* y *Ahora se juega* se desplazan a la fila elegida y `perf.rs` mide por proporción y tope (release, 5.000 filas: refresco automático 58 ms con la poda en una pasada y la actualización sin recorrer el orden, filtro 9 ms, orden 3,5 ms, *Buscar jugador* 4×5.000 136 ms, 3.850 B por fila); la rama `rust` se subió y su CI quedó verde en Linux y Windows (primera suite completa en Windows: 2361 bien, 1 ignorado) tras arreglar el reintento del *pipe* y los datos de referencia con CRLF.
- Descartado: topes finos de tiempo en el CI, construir accesibilidad propia (Iced 0.14 sin AccessKit) y tomar las decisiones del dueño ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la [lista del dueño](../hoja-de-ruta/pruebas-dueno-rust.md) (KDE, Windows real y las divergencias por decidir) y la fase 6, empaquetado ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-19 — port a Rust, fase 5i (vigilancias, herramientas y bandeja)
- La app Iced pasa a `iced::daemon`: *Buscar jugador*, *Ahora se juega* y cada consola RCON son ventanas aparte (suscripciones filtradas por `window::Id`, que también cierra la deuda del cierre de la 5a); vigilar sitio, umbral de partida, amigos y su revisión de fondo van en `Task` abortables con número de vuelta; la clave RCON vive en el llavero y en `Secret`; la bandeja usa `ksni` + `zbus` en Linux y `Shell_NotifyIconW` en Windows (5 paquetes nuevos, solo en Linux); `--screenshot` saca además las tres herramientas en su ventana, *Amigos* y el umbral, capturando sin un rearmado de la vista en medio (si no, el `text_editor` sale vacío).
- Descartado: paneles dentro de la ventana para las herramientas, `tray-icon` (GTK y sin globos), StatusNotifierItem a mano y `notify-rust` ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la bandeja real en KDE y Windows (solo la puede probar el dueño), las divergencias que decide el dueño y la 5j ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-19 — port a Rust, fase 5h (ajustes y tareas de fondo)
- La app Iced tiene Ajustes con sus cuatro pestañas (rutas con estado, masters, general, apariencia), *Buscar juegos* y el ofrecimiento del primer arranque, masters diarios, base de países, refresco automático y aviso y descarga de versiones nuevas, todo con la E/S en `Task` y número de pedido; quedaron hechos *Abrir ajustes* de los avisos y guardar la clave de Steam (pendientes desde la 5f); `--screenshot` saca además Ajustes y *Buscar juegos* y se midieron de Qt la columna de nombres, el alto de las páginas, las filas de masters y la rejilla de *Buscar juegos*.
- Descartado: un widget de pestañas de terceros, una pila de `Dialog` para los diálogos internos y comparar versiones contra `0.1.0` (`update_version()` = `dev` mientras el crate sea 0.x) ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la bandeja usa `minimize_to_tray` y *Buscar jugador* pone la selección pendiente en la 5i; la actualización real en Windows; la fase 7 sube a 2.0.0 y quita el `shortcut:` de `update_version` ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-18 — port a Rust, fase 5g (agregar servidor, enlaces e instancia única)
- La app Iced agrega servidores (`Ctrl+N`, `Ctrl+V`, *Detectar*, lista de direcciones) con la lógica de `add_server.py`, exporta, importa y copia favoritos (`rfd` sin GTK), copia y abre enlaces `jase://` con confirmación en texto plano y es instancia única con el mismo socket/*pipe* que 1.3.0 (en Linux solo si el socket es del usuario); `--screenshot` saca además «Agregar servidor» y se midieron de Qt el formulario y los botones de los diálogos; en Windows nativo compila sin warnings.
- Descartado: `regex` para las direcciones, `AsyncFileDialog` con ventana padre y el socket en `$XDG_RUNTIME_DIR` (no vería a una 1.3.0 abierta) ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: `_pending_select` con el refresco automático en la 5h; traer la ventana desde la bandeja en la 5i; probar el *pipe* en el CI de Windows; registrar `jase://` para el binario de Rust en la fase 6 ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-18 — port a Rust, fase 5f (conectar y favoritos)
- La app Iced conecta (doble clic, Enter, menús) con la contraseña del llavero (`keyring-core` + stores de Secret Service por `zbus` y del Administrador de credenciales, compatibles con lo que guardó 1.3.0) y lanza en una `Task`; lee la clave de Steam en un hilo; tiene las vistas Favoritos y Recientes (pedido numerado, 64 consultas a la vez, filtros apagados), la estrella, notas, grupos y recientes; `--screenshot` saca además el diálogo de contraseña y se midieron de Qt la lista lateral, el orden de los botones del diálogo y el color de los controles apagados.
- Descartado: `keyring` 3 con `async-secret-service` (un segundo `zbus`, 50 paquetes), la feature `tokio` de `zbus` y escribir como Python en el destino del servicio en Windows ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: *Abrir ajustes* y guardar la clave de Steam en la 5h; agregar servidor y copiar favoritos en la 5g; probar el llavero en el CI de Windows ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-18 — port a Rust, fase 5e (ficha del servidor)
- La app Iced tiene la ficha derivada del servidor elegido con un gancho `sync_card` (imagen del mapa en `spawn_blocking` con 2 permisos y número de pedido, `image-without-codecs`), jugadores ordenables y reglas, divisiones `pane_grid` guardadas en `rs_right_splitter`/`rs_details_splitter`, historial de ocupación leído en un hilo (no se guarda antes de leerse; los guardados van en orden) con su gráfico por hora local, e icono del exe en el árbol solo en Windows; `--screenshot` ordena los jugadores por puntos y se midieron de Qt la división, la barra vertical que ocupa lugar solo si hace falta, los títulos en negrita solo con selección, los botones de Luna y la barra de estado de 98/VGUI.
- Descartado: `canvas` para el gráfico, `image` con códecs, `Handle::from_path` y decodificar en el render ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la estrella y la nota editable en la 5f; volver a pedir la imagen al aceptar Ajustes en la 5h; las divergencias que decide el dueño ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-18 — port a Rust, fase 5d (menús, atajos y ventana)
- La app Iced tiene barra de menús, menú contextual y de tema con un menú propio sobre `overlay()` (acciones como datos; las de subfases futuras, apagadas), barra de herramientas, atajos por foco, barra de estado con progreso, búsqueda en la red local, refrescar un servidor (`ScanState`) y columnas, barra lateral y ventana guardadas en claves `rs_`; `--screenshot` deja *País* a 70 px y se midieron de Qt la barra de menús y de estado, el encabezado, los ítems y la columna de casilla o icono de los menús y los márgenes de celda.
- Descartado: el menú en una capa de `stack` (no sabe dónde quedó el ancla), `iced_aw` y medir cada celda para el «…» ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: cada subfase enciende sus acciones (5e–5i); las divergencias que decide el dueño (celdas sin «…», menús sin flechas, radio de los menús planos) ([deuda](../hoja-de-ruta/deuda.md)).

## 2026-11-18 — port a Rust, fase 5c (barra lateral y filtros)
- La app Iced tiene el árbol de juegos (Mods, Mapas, Países, marcas por juego, solo instalados), los filtros rápidos, de texto y de versión en el modelo de la tabla, el país por fila con la base DB-IP, el modelo de foco, los filtros guardados y las listas guardadas entre sesiones (escritor en orden que sobrevive a un pánico); `--screenshot` abre el árbol y se corrigió lo que difería de 1.3.0 (alto de fila por tipo, márgenes de Qt, banderas 16×12, combos de 16 px de línea, barra de estado de 22 px).
- Descartado: el `checkbox` de Iced (no se puede pintar como Qt; casilla propia en `widgets.rs`) y una caché de países por host con `RefCell` (país guardado por fila) ([diseño](../hoja-de-ruta/port-rust.md)).
- Abierto: la 5d muda el campo y la versión a la barra de herramientas y cambia el botón de filtros guardados por su menú; las divergencias que acepta el dueño ([deuda](../hoja-de-ruta/deuda.md)).

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

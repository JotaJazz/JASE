# Port a Rust: pruebas a mano y decisiones del dueño

Lo que solo puede probar o decidir el dueño antes de la 2.0 (cierre de la fase 5, 5j). Lo
probado en WSL y por tests está en [port-rust.md](port-rust.md) (tabla «Paridad de la app»)
y en [deuda.md](deuda.md). Al terminar un paso, marcar su casilla; un fallo va a
[deuda.md](deuda.md) o a una tarea.

## Ya probado por el dueño

En Windows nativo, el 2026-11-18 con `ab65b09`: compila con MSVC; los iconos de los exes en el
árbol, el estado de la ventana (tamaño y posición) y la lectura de una contraseña guardada en
el llavero desde Rust (el diálogo la trae precargada) funcionan. La suite corrió una vez allá
antes de la 5g: 1275 ok y 3 fallos, arreglados en `8103580`; desde entonces el antivirus corta
los binarios de test (recomendación: excluir la carpeta del clon en el antivirus; la suite de
Windows ya la corre el CI). El último `jase.exe` compilado allá es de `fc8ee98` (fin de la
5i): recompilar antes de probar.

## 1. El CI (hecho)

El CI corre en cada *push* de cualquier rama y no publica nada (la release sale solo de un
tag).

- [x] Subir la rama, sin release (2026-11-19, a pedido del dueño). Los dos primeros CI
  fallaron en Windows y se arreglaron: el test del *pipe* no reintentaba con
  `ERROR_PIPE_BUSY` (`c5c6390`) y el checkout convertía a CRLF los datos de referencia de
  `rust/jase-core/tests/golden/` (`a63e29f`: `-text` en `.gitattributes`).
- [x] El CI de `a63e29f` (run 44606966939) quedó todo verde: `ubuntu-latest / Rust` en 15 min,
  `windows-latest / Rust` en 21 min (lejos del tope de 60) y Python en Linux y Windows. Es la
  primera suite completa de Rust en Windows: 2361 bien, 0 fallos y 1 ignorado (el llavero
  real, `real_keyring_round_trip`).
- [x] Cubierto desde entonces por el job de Windows: rutas, hora local, `Ctrl+Q` solo fuera
  de Windows, `explorer_has_an_icon` (icono de exe en el árbol), el llavero compatible con
  1.3.0 sobre el mapa en memoria, el *pipe* de instancia única, la bandeja y la
  actualización compiladas, ICMP de puerto cerrado (`ConnectionReset`), el registro y el
  script portátil con `cmd.exe`. Lo que el CI no puede probar (bandeja y globos reales,
  llavero real, autoactualización, enlaces y *pipe* con una JASE abierta de verdad) sigue en
  las pruebas a mano de abajo.

## 2. Pruebas a mano

Siempre desde `rust/`. En Linux, `cargo run --release -p jase`; en Windows, `cargo build
--release -p jase` y correr el `jase.exe` de la carpeta `target\release` (o la de
`--target-dir`). Para la bandeja: Ajustes → General → «Minimizar a la bandeja del sistema» y
*Guardar*. Para los globos: clic derecho en un servidor lleno → *Avisar cuando haya sitio* (o
un umbral con *Avisar cuando se arme partida*) y minimizar. Los globos solo salen con la
ventana oculta en la bandeja: en Wayland, donde minimizar no oculta, no hay globos.

### KDE Plasma con Wayland (Linux nativo, sin WSL)

- [ ] La ventana abre sin «Broken pipe» ni otro error de Wayland en la terminal; cerrarla
  termina el proceso enseguida. Si no abre, el respaldo X11 (`force_x11` también cuando
  Wayland falla) entra como tarea aparte.
- [ ] Escanear CoD2, ordenar por 3 columnas y desplazar 1.000+ filas con la rueda, la barra y
  Re Pág/Av Pág: sin tirones.
- [ ] `cargo run --release -p jase -- --smoke` con `ICED_BACKEND=tiny-skia` y sin él: sale
  con 0 (`echo $?`).
- [ ] Tema *Sistema*: cambiar el modo claro/oscuro de Plasma con JASE abierta; la ventana
  cambia en vivo.
- [ ] Llavero (KWallet o el de GNOME por Secret Service): conectar a un servidor con
  contraseña y marcar guardarla; cerrar, abrir y conectar de nuevo: vuelve precargada.
  *Olvidar contraseña* en el diálogo la borra («Contraseña olvidada» en la barra). Lo mismo
  con la clave de la consola RCON y la clave de Steam en Ajustes. Y
  `cargo test -p jase --test passwords -- --ignored` (corre `real_keyring_round_trip`):
  pasa.
- [ ] Bandeja: icono con el tooltip «JASE»; menú *Mostrar JASE* / *Salir*; un clic simple
  trae la ventana (en KDE es clic simple, no doble); minimizar **no** oculta la ventana
  (esperado en Wayland), así que **no hay globos**: «Hay sitio» abre la pregunta en la ventana
  y «Amigo en línea» va a la barra de estado; *Salir* no deja un icono fantasma.
- [ ] Selector de archivos (portal): *Exportar favoritos…* guarda un JSON; *Importar
  favoritos…* lo lee; *Examinar…* en Ajustes y *Buscar en carpeta…* en *Buscar
  juegos* abren y devuelven la carpeta elegida.
- [ ] Portapapeles: `Ctrl+C` con un servidor elegido y pegar en otro programa da
  `IP:puerto`; *Copiar enlace jase://* da el enlace; copiar `198.51.100.7:28960` en otro
  programa: `Ctrl+V` en la tabla y `Ctrl+N` abren «Agregar servidor» con esa dirección ya
  escrita.
- [ ] Conectar a un juego de Steam abre su enlace `steam://` con `xdg-open` (*Ver la página*
  del aviso de versión nueva no sale hasta la 2.0.0).
- [ ] Instancia única: con JASE abierta, correr

  ```
  cargo run --release -p jase -- "jase://connect/cod2/<ip>:<puerto>"
  ```

  La primera elige el servidor y pregunta si conectar; la segunda sale enseguida.

### KDE Plasma con X11

- [ ] La bandeja como en Wayland, pero minimizar **sí** oculta la ventana y el clic en el
  icono la trae; con la ventana oculta, el globo «Hay sitio» sale y su clic trae la ventana
  con la pregunta.
- [ ] Mover la ventana, cerrar y abrir: vuelve en la misma posición y tamaño (en WSLg la
  decide el compositor y no se pudo probar).

### Windows

- [ ] Bandeja: icono con tooltip, menú *Mostrar JASE* / *Salir*, doble clic trae la ventana,
  minimizar la oculta, el globo «Hay sitio» y su clic traen la pregunta, *Salir* sin icono
  fantasma. Con el icono visible, reiniciar el Explorador (Administrador de tareas →
  *Explorador de Windows* → *Reiniciar*): el icono vuelve.
- [ ] Iconos de los exes en el árbol (ya visto el 2026-11-18; repetir con la versión nueva).
- [ ] Llavero: guardar y borrar una contraseña (*Olvidar contraseña*) desde Rust; la lectura
  ya se probó. Contraseña al revés: cambiarla en Rust, abrir la 1.3.0 y conectar a ese
  servidor: la 1.3.0 la trae precargada.
- [ ] Instancia única por el *pipe*: con una 1.3.0 abierta y luego con una Rust abierta,
  `jase.exe "jase://connect/cod2/<ip>:<puerto>"` → la abierta pregunta y la nueva sale; y
  `jase.exe` solo, con la abierta minimizada o en la bandeja → vuelve (anotar si solo parpadea
  el botón de la barra de tareas).
- [ ] Selector de archivos: exportar/importar favoritos, *Examinar…* y *Buscar en carpeta…*
  abren; anotar si el diálogo queda detrás de la ventana.
- [ ] Portapapeles: `Ctrl+C`, *Copiar enlace jase://*, `Ctrl+V` y `Ctrl+N` como en KDE.
- [ ] Tahoma: `jase.exe --screenshot <carpeta>` (antes, `set ICED_BACKEND=tiny-skia` en cmd o
  `$env:ICED_BACKEND="tiny-skia"` en PowerShell) y mirar el
  marco de «00/00» en la columna *Jugadores* y las capturas de los temas 98 y XP.
- [ ] Tema *Sistema* con el modo oscuro de Windows en vivo.
- [ ] `Ctrl+Q` no sale (en Windows no existe; se sale con *Archivo → Salir* o `Alt+F4`).
- [ ] Lanzar los juegos instalados de la tabla «Sin verificar en Windows real» de
  [deuda.md](deuda.md) con doble clic.
- [ ] Autoactualización: solo compilada; con `update_version() = dev` no ofrece nada hasta
  que la fase 7 publique la 2.0.0. Queda para la fase 7: con la 2.0.0 publicada y una 2.0.1 de
  prueba, *Buscar actualizaciones…* → descargar → el instalador (o el script del portátil)
  reemplaza el exe y JASE vuelve a abrir.

## 3. Decisiones pendientes

Reunidas de las filas 5b–5i de [deuda.md](deuda.md) y del recorrido de la 5j. La
recomendación es la del plan de la 5j; decide el dueño.

### Teclado y lectura: recomendado arreglar antes de la 2.0

| Qué | Opciones | Recomendación |
|---|---|---|
| Tab no recorre los campos de un diálogo (Iced trae `focus_next`, sin conectar) | conectar Tab/Shift+Tab / dejarlo | arreglar antes de la 2.0 |
| Los menús no se recorren con flechas ni tienen mnemónicos; no se elige con el botón derecho (5d) | widget de menú con teclado / dejarlo | arreglar antes de la 2.0 |
| El botón por defecto de los diálogos sin marcar (Qt resalta *Conectar*, *Guardar*… en 98, Luna, VGUI y Terminal) (5f, 5j) | marcarlo con el borde del tema / dejarlo | arreglar antes de la 2.0 |
| La lista lateral (★ Favoritos, 🕘 Recientes) no se maneja con el teclado (5f) | flechas y Enter / dejarlo | arreglar antes de la 2.0 |
| Las pestañas de Ajustes sin `Ctrl+Tab`, la lista de temas sin flechas, Enter sin efecto en la descarga (5h) | con Tab / dejarlo | arreglar junto con Tab |
| Celdas largas de la tabla cortadas sin «…» (5d); nombres largos del árbol sin «…» al medio (5c) | medir solo las celdas cuyo texto no entra / dejarlo | arreglar antes de la 2.0 |
| ↑/↓ de *Buscar jugador* desde el campo (5i) | recorrer la tabla desde el campo / dejarlo | arreglar junto con Tab |

### Límite de Iced o de los catálogos: recomendado aceptar

| Qué | Opciones | Recomendación |
|---|---|---|
| Insignia apagada del juego elegido en el árbol (5b; 1.3.0 también la apaga) | encenderla / dejarla | aceptar |
| Lista del combo *País* sin banderas (5c) | widget propio / aceptar | aceptar |
| Radio marcado de los menús planos: punto redondo (1.3.0, cuadrado) (5d) | dibujarlo / aceptar | aceptar |
| Jugadores y reglas sin selección de filas (5e; en Qt no hace nada más) | seleccionar / aceptar | aceptar |
| Borde doble de Terminal como uno simple de 3 px; recuadro del mapa achicado en vez de recortado (5e) | dibujarlo / aceptar | aceptar |
| Asa de los divisores sin la trama de puntos de Fusion; 🔒 sin color si la letra no lo trae (5e) | dibujarla / aceptar | aceptar |
| Aviso del llavero después de lanzar el juego (5f) | avisar antes / aceptar | aceptar |
| Enlace `jase://` y «Hay sitio» sin el icono de pregunta de `QMessageBox` (5g, 5j) | icono / aceptar | aceptar |
| *Guardar*/*Cancelar* y *Conectar*/*Cancelar* en vez de *OK*/*Sí*/*No* (*Agregar master*, *Editar nota…*, *Amigos*, el umbral, «Hay sitio»); *Cerrar* en los avisos (5h, 5i, 5j) | claves nuevas en los catálogos / aceptar | aceptar |
| Etiquetas de «Agregar servidor» centradas con su campo (Qt las sube 1–3 px) (5g) | ajustar / aceptar | aceptar |
| Pestañas de Ajustes sin solaparse con el borde del panel; en Luna sin la línea naranja de la elegida; borde de las muestras de temas con el del tema activo; enlace de versión nueva sin bisel (5h) | dibujarlo / aceptar | aceptar |
| Herramientas como ventanas que no quedan encima de la principal (se traen con la barra de tareas) (5i) | ventanas hijas / aceptar | aceptar |
| «Hay sitio» a la barra de estado con otro diálogo abierto (5i) | encolarlo / aceptar | aceptar |
| El umbral como tarjeta con el texto partido y sin flechas; filas altas de la lista de *Amigos* (5i) | ajustar / aceptar | aceptar |
| Clave de la consola RCON con asteriscos (Qt, puntos) (5i) | puntos / aceptar | aceptar |

### Plataforma: recomendado aceptar con nota en la guía 2.0

| Qué | Opciones | Recomendación |
|---|---|---|
| En KDE el icono de la bandeja responde al clic simple (Windows, doble) (5i) | igualar / anotar | aceptar y anotarlo en la guía 2.0 |
| En Wayland minimizar no oculta a la bandeja (y por eso no hay globos) y no se guarda la posición de la ventana (5d, 5i) | — (el sistema no lo da) | aceptar y anotarlo en la guía 2.0 |
| Accesibilidad: sin lector de pantalla (abajo) | esperar a Iced / otra interfaz | aceptar, revisar en cada versión de Iced |

## 4. Empaquetado (fase 6)

Ya cubierto por el CI (2026-11-19, `dec22c4`, *Package* y CI verdes): el AppImage en
`ubuntu-22.04` con su prueba de humo; el zip y el instalador de Windows con la suya; el
instalador sobre la 1.3.0 publicada (misma carpeta, una sola entrada de desinstalación,
`jase://` al `JASE.exe` de Rust, sin `_internal\PySide6`, desinstalar sin tocar
`%APPDATA%\JimmysASE`) y el portátil 1.3.0 actualizado con el `portable_update.py` de
`v1.3.0`. El exe ya se compiló en el Windows del dueño (VersionInfo, icono, subsistema de
ventana, sin `VCRUNTIME140`).

- [ ] Tras subir la rama, mirar los jobs `windows` y `linux` de *Package*
  (`gh run list --workflow package.yml --branch rust`) y bajar los paquetes con
  `gh run download <id>`.

Con los paquetes `JASE-0.1.0-*` bajados del run de *Package* (nunca de una release):

- [ ] **Instalar sobre una 1.3.0 real** (su Windows): con JASE 1.3.0 instalada y cerrada, abrir `JASE-0.1.0-setup.exe`: propone la carpeta de siempre; al terminar, *Configuración → Aplicaciones* muestra una sola JASE (0.1.0); el acceso del menú Inicio abre la de Rust con la config de siempre (juegos, favoritos, contraseñas del llavero); un enlace `jase://` desde el navegador abre la de Rust; la carpeta de instalación ya no tiene los DLL de PySide6 (solo `_internal\README.txt`).
- [ ] **Con JASE 1.3.0 abierta**, correr el setup nuevo: Inno pide cerrarla (o la cierra) y termina bien.
- [ ] **Desinstalar**: desaparece la carpeta de *Archivos de programa* (o la de usuario), el acceso y la entrada de *Aplicaciones*; `%APPDATA%\JimmysASE` sigue.
- [ ] **Volver a 1.3.0**: reinstalar `JASE-1.3.0-setup.exe` sobre la de prueba; abre la 1.3.0 con la config intacta (así sigue usando la publicada).
- [ ] **Portátil sobre portátil**: copiar su carpeta portátil 1.3.0 y descomprimir encima el `JASE-0.1.0-win64.zip` con «Reemplazar todo»: abre la de Rust; borrar esa copia después. (La actualización desde la app la prueba el CI con el código de 1.3.0; la real, con la 2.0.0 en la fase 7.)
- [ ] **El exe**: icono en el Explorador, la barra de tareas y el acceso; *Propiedades → Detalles* con «Jimmy's All Seeing Eye» y 0.1.0; doble clic **no** abre una consola; SmartScreen avisa (sin firma, como 1.3.0) y el antivirus no lo borra.
- [ ] **Linux (KDE)**: el AppImage abre con `chmod +x` y doble clic, con su icono, y la barra de tareas lo agrupa bajo ese icono (`WM_CLASS` `JASE`, como `StartupWMClass`); con AppImageLauncher (si lo usa), queda en el menú.
- [ ] **Fase 7, de punta a punta**: una 1.3.0 instalada y una portátil ven la 2.0.0 publicada, la bajan y quedan en Rust (antes de marcarla *latest*: una prerelease ya la ven todos los 1.3.0).

## Accesibilidad

Iced 0.14 no integra AccessKit: `accesskit` no está en el `Cargo.lock` y `iced_winit` 0.14.1 no
conecta el de `winit` 0.30; `svg` e `image` no tienen texto alternativo. Narrador, NVDA y Orca
solo ven la ventana y su título. Lo que sí hay: atajos de teclado y el foco propio (árbol,
tabla, campo, diálogo), escala por DPI del sistema (`winit`), temas de alto contraste
(Terminal, Clásico; contraste de `theme.py` comprobado en `rust/jase/tests/theme.rs`) y
tooltips. `operation::focus_next`/`focus_previous` existen pero no están conectados (Tab no
recorre campos: primera fila de las decisiones). Se revisa en cada versión de Iced (fila de
deuda aceptada en [deuda.md](deuda.md)).

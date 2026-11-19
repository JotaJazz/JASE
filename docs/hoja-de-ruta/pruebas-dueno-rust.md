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
los binarios de test (recomendación: excluir la carpeta del clon en el antivirus). El último
`jase.exe` compilado allá es de `fc8ee98` (fin de la 5i): recompilar antes de probar.

## 1. Antes de nada: el CI

La rama `rust` nunca se subió (`gh run list --branch rust` está vacío): todo lo que
[deuda.md](deuda.md) atribuye a «el CI de Windows» no corrió nunca. El CI corre en cada *push*
de cualquier rama y no publica nada (la release sale solo de un tag).

- [ ] Decidir subir la rama, sin release: `git push -u origin rust`.
- [ ] En GitHub → *Actions* → *CI* de la rama `rust`: los jobs `ubuntu-latest / Rust` y
  `windows-latest / Rust` en verde, cada uno en menos de 60 min (el tope del job). Si Windows
  pasa de 45 min, agregar caché de cargo (fila del CI en [deuda.md](deuda.md)).
- [ ] Lo que prueba por primera vez el job de Windows: la suite completa en Windows (rutas,
  hora local, `Ctrl+Q` solo fuera de Windows), `explorer_has_an_icon` (icono de exe en el
  árbol), el llavero con el mapa en memoria, el *pipe* de instancia única, la bandeja y la
  actualización compiladas, ICMP de puerto cerrado (`ConnectionReset`), el registro y el
  script portátil con `cmd.exe`. Si algo falla, el registro del job dice qué test.

## 2. Pruebas a mano

Siempre desde `rust/`. En Linux, `cargo run --release -p jase`; en Windows, `cargo build
--release -p jase` y correr el `jase.exe` de la carpeta `target\release` (o la de
`--target-dir`). Para la bandeja: Ajustes → General → «Minimizar a la bandeja del sistema» y
*Guardar*. Para los globos: clic derecho en un servidor lleno → *Avisar cuando haya sitio* (o
un umbral con *Avisar cuando se arme partida*) y minimizar.

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
  con la clave de la consola RCON y la clave de Steam en Ajustes. Y `cargo test -p jase --test passwords -- --ignored` (corre
  `real_keyring_round_trip`): pasa.
- [ ] Bandeja: icono con el tooltip «JASE»; menú *Mostrar JASE* / *Salir*; un clic simple
  trae la ventana (en KDE es clic simple, no doble); minimizar **no** oculta la ventana
  (esperado en Wayland); el globo «Hay sitio» sale y su clic trae la ventana con la pregunta;
  *Salir* no deja un icono fantasma.
- [ ] Selector de archivos (portal): *Exportar favoritos…* guarda un JSON; *Importar
  favoritos…* lo lee; *Examinar…* en Ajustes y *Buscar en carpeta…* en *Buscar
  juegos* abren y devuelven la carpeta elegida.
- [ ] Portapapeles: `Ctrl+C` con un servidor elegido y pegar en otro programa da
  `IP:puerto`; *Copiar enlace jase://* da el enlace; copiar `198.51.100.7:28960` en otro
  programa: `Ctrl+V` en la tabla y `Ctrl+N` abren «Agregar servidor» con esa dirección ya
  escrita.
- [ ] Conectar a un juego de Steam abre su enlace `steam://` con `xdg-open` (*Ver la página*
  del aviso de versión nueva no sale hasta la 2.0.0).
- [ ] Instancia única: con JASE abierta, `cargo run --release -p jase --
  "jase://connect/cod2/<ip>:<puerto>"`: la primera elige el servidor y pregunta si conectar;
  la segunda sale enseguida.

### KDE Plasma con X11

- [ ] La bandeja como en Wayland, pero minimizar **sí** oculta la ventana y el clic en el
  icono la trae.
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
- [ ] Tahoma: `jase.exe --screenshot <carpeta>` (con `ICED_BACKEND=tiny-skia`) y mirar el
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
| En Wayland minimizar no oculta a la bandeja y no se guarda la posición de la ventana (5d, 5i) | — (el sistema no lo da) | aceptar y anotarlo en la guía 2.0 |
| Accesibilidad: sin lector de pantalla (abajo) | esperar a Iced / otra interfaz | aceptar, revisar en cada versión de Iced |

## Accesibilidad

Iced 0.14 no integra AccessKit: `accesskit` no está en el `Cargo.lock` y `iced_winit` 0.14.1 no
conecta el de `winit` 0.30; `svg` e `image` no tienen texto alternativo. Narrador, NVDA y Orca
solo ven la ventana y su título. Lo que sí hay: atajos de teclado y el foco propio (árbol,
tabla, campo, diálogo), escala por DPI del sistema (`winit`), temas de alto contraste
(Terminal, Clásico; contraste de `theme.py` comprobado en `rust/jase/tests/theme.rs`) y
tooltips. `operation::focus_next`/`focus_previous` existen pero no están conectados (Tab no
recorre campos: primera fila de las decisiones). Se revisa en cada versión de Iced (fila de
deuda aceptada en [deuda.md](deuda.md)).

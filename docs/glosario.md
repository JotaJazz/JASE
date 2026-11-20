# Glosario

| Término | Significado |
|---|---|
| Master | servidor que publica la lista de servidores de un juego |
| Familia | protocolo común a varios juegos (`quake3`, `quake2`, `gamespy`, `valve`, `ut2004`, `zandronum`) |
| `GameDef` | definición de un juego en `rust/jase-core/src/games_table.rs`: familia, masters, exe, lanzamiento, filtros |
| Dirección de consulta / de conexión | `ServerInfo.address` (la que se consulta y guardan Favoritos) / `connect_address` (la que recibe el juego) |
| Lista completa | escaneo terminado sin error o restaurado de una caché; la única que reemplaza la guardada y poda marcas |
| Lista de la sesión | `App::cache` en memoria, por juego |
| Lista guardada | la del último escaneo completo en `cache/servers/` (ADR 0006) |
| Vista directa | Favoritos o Recientes: consulta sus direcciones sin master |
| Marcas del árbol | mods, mapas y países marcados en el árbol que filtran la tabla |
| Filtros rápidos | ocultar vacíos, llenos, con contraseña, solo bots, ping máximo, país |
| Generación | contador que invalida resultados tardíos de un escaneo o búsqueda anterior |
| `Message` | lo que llega a `update` de Iced: un clic, una tecla o el resultado de un `Task` (red o disco) |
| `Task` | trabajo de fondo de Iced (tokio o `spawn_blocking`) cuyo resultado vuelve como `Message` |
| Sondeo | consulta real a un master y a una muestra de sus servidores (`jase-cli scan <id>`) |
| Captura | respuesta real de un servidor guardada en `tests/fixtures/*.bin` |
| Badge | icono dibujado con las iniciales del juego cuando no hay icono del exe |
| `jase://` | enlace `jase://connect/<juego>/<host>:<puerto>` que abre JASE y pide confirmar |

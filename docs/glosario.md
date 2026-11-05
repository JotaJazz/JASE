# Glosario

| Término | Significado |
|---|---|
| Master | servidor que publica la lista de servidores de un juego |
| Familia | protocolo común a varios juegos (`quake3`, `quake2`, `gamespy`, `valve`, `ut2004`, `zandronum`) |
| `GameDef` | definición de un juego en `src/jase/games.py`: familia, masters, exe, lanzamiento, filtros |
| Dirección de consulta / de conexión | `ServerInfo.address` (la que se consulta y guardan Favoritos) / `connect_address` (la que recibe el juego) |
| Lista completa | escaneo terminado sin error o restaurado de una caché; la única que reemplaza la guardada y poda marcas |
| Lista de la sesión | `_cache` en memoria, por juego |
| Lista guardada | la del último escaneo completo en `cache/servers/` (ADR 0006) |
| Vista directa | Favoritos o Recientes: consulta sus direcciones sin master |
| Marcas del árbol | mods, mapas y países marcados en el árbol que filtran la tabla |
| Filtros rápidos | ocultar vacíos, llenos, con contraseña, solo bots, ping máximo, país |
| Generación | contador que invalida resultados tardíos de un escaneo o búsqueda anterior |
| Relay | QObject sin padre que lleva la señal de un hilo daemon al hilo de la UI |
| Mixin | `src/jase/ui/window_*.py`: parte de `MainWindow` por función, no es QObject |
| Sondeo | consulta real a un master y a una muestra de sus servidores (`scripts/probe_*.py`) |
| Captura | respuesta real de un servidor guardada en `tests/fixtures/*.bin` |
| Badge | icono dibujado con las iniciales del juego cuando no hay icono del exe |
| `jase://` | enlace `jase://connect/<juego>/<host>:<puerto>` que abre JASE y pide confirmar |

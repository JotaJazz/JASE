# Visión general

JASE es un navegador de servidores de juegos clásicos: pide la lista a los masters,
consulta cada servidor por UDP y lanza el juego conectado. Una app de escritorio
(Iced) y una CLI sin interfaz comparten el mismo núcleo, en un workspace de Cargo en `rust/`.

## Capas

```
rust/
├── jase-core/src/   núcleo sin UI (sin Iced, keyring ni imágenes por defecto)
│   ├── protocols/   parsers puros por familia (bytes → ServerInfo / direcciones); sin red
│   ├── scanner/     tokio: masters (UDP, TCP, Steam) + consultas UDP en paralelo
│   ├── games.rs     GameDef por juego (familia, masters, exe, lanzamiento, filtros);
│   │                la tabla en games_table.rs
│   ├── models.rs    ServerAddress, ServerInfo, Player, QuickFilters
│   ├── núcleo       config, masters, launcher, detect, identify, lan, rcon, geo, history,
│   │                server_cache, app_update, portable_update, i18n, …
│   │                paths.rs: exe y carpetas anidadas (sin distinguir mayúsculas)
│   │                browse.rs: mods, mapas y países de una lista (desglose del árbol)
│   │                net.rs: descargas HTTP pequeñas con plazo y tope de tamaño
│   └── maps/        imágenes de mapas (IWI de CoD, .unr de UT99, levelshots, GameTracker);
│                    la decodificación, con la feature map-images
├── jase-cli/        jase-cli: JSON versionado (contrato en docs/cli.md)
└── jase/src/        Iced: app/ (estado, update, view por dominio), table/, theme/, tray/,
                     diálogos y ventanas, llavero, instancia única
```

Detalle de cada capa: [protocolos.md](protocolos.md), [ui.md](ui.md). Reglas de
dependencia: [limites.md](limites.md).

## Flujo principal: escanear un juego

1. `rust/jase/src/app/saved.rs` (`select_game`): lista de la sesión, la guardada en disco
   (`server_cache`, en `spawn_blocking`) o un escaneo nuevo.
2. `rust/jase/src/scan.rs`: el escaneo de `jase-core` como `Stream` de un `Task` de Iced,
   con su número de generación (`ScanState`).
3. `scanner::scan::scan`: `fetch_master` por cada master (en paralelo), deduplica
   direcciones y `query_servers` las consulta por un socket UDP compartido (64 a la vez,
   reintento).
4. Cada respuesta válida → `protocols/<familia>.rs` → `ServerInfo` → `ScanEvent` como
   `Message` → `update` (`rust/jase/src/app/scanning.rs`, descarta otra generación) →
   `table::model` (`add_or_update`).
5. Escaneo completo sin error: la lista pasa a la caché de la sesión y a
   `cache/servers/<juego>.json` (ver [ADR 0006](adr/0006-listas-guardadas.md)).
6. Doble clic → `launcher::launch`: argv según `GameDef.launch_style`, contraseña del
   llavero (`rust/jase/src/passwords.rs`).

## Datos que se guardan

Los mismos archivos y formatos que 1.3.0: una instalación que pasa a la 2.0.0 conserva todo.

| Qué | Dónde | Módulo |
|---|---|---|
| Configuración (rutas, favoritos, filtros, estado de la ventana) | `%APPDATA%\JimmysASE\config.json` o `~/.config/jimmys-ase/config.json` | `config.rs` |
| Contraseñas de servidor y RCON, clave de Steam | llavero del sistema (servicio `JimmysAllSeeingEye`) | `rust/jase/src/passwords.rs` |
| Masters personales | `masters.custom.json` junto a la config | `masters.rs` |
| Lista oficial de masters descargada | `cache/masters.json` + meta | `masters.rs` |
| Listas de servidores entre sesiones | `cache/servers/` | `server_cache.rs` |
| Historial de ocupación (favoritos, recientes) | `cache/history.json` | `history.rs` |
| Base de países (DB-IP Lite) | `cache/dbip-country-lite.csv` | `geo.rs` |
| Imágenes de mapas | `cache/maps/` | `rust/jase-core/src/maps/service.rs` |
| Descargas de actualización | `cache/updates/` | `app_update.rs`, `portable_update.rs` |

Todo se escribe de forma atómica (archivo temporal + renombrar; `fsutil::atomic_write`) y
se carga de forma tolerante: cada campo o entrada inválida vuelve a su valor por defecto sin
tumbar el resto.

## Ejecutables

| Comando | Crate | Necesita |
|---|---|---|
| `jase` (`JASE.exe` en el paquete) | `rust/jase` | Iced, keyring, imágenes (`map-images`) |
| `jase-cli` | `rust/jase-cli` | solo `jase-core`; no va en los paquetes |

Empaquetado: instalador Inno Setup (`packaging/jase.iss`), zip portátil
(`scripts/package_windows.ps1`) y AppImage (`scripts/package_linux.sh`, que usa
`scripts/build_appimage.sh`), en `.github/workflows/package.yml`.

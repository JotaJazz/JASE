# Límites (constitución de la arquitectura)

Reglas normativas. Romper una exige un ADR que la reemplace ([adr/](adr/README.md)).

## Dependencias permitidas

Módulos de `rust/jase-core/src/` y los otros dos crates del workspace:

| Desde \ hacia | `protocols` | `models` | `games` | núcleo | `maps` | `jase` (Iced) | Iced | keyring / `image` |
|---|---|---|---|---|---|---|---|---|
| `protocols` | sí | sí | no | solo `pystr`, `pyjson` | no | no | no | no |
| `scanner` | sí | sí | sí | `i18n`, `masters`, `net`, … | no | no | no | no |
| núcleo (`config`, `masters`, `launcher`, `server_cache`, …) | sí | sí | sí | sí | no | no | no | no |
| `maps` | no | sí | sí | `net`, `paths`, `fsutil` | sí | no | no | `image` solo con la feature `map-images` |
| `jase-cli` | sí | sí | sí | sí | no | no | **no** | **no** |
| `jase` | sí | sí | sí | sí | sí | sí | sí | sí (el llavero vive en `rust/jase/src/passwords.rs`) |

`jase-core` no depende de Iced, keyring ni imágenes en sus features por defecto: lo vigila
`rust/jase/tests/deps.rs` con `cargo tree` ([ADR 0001](adr/0001-nucleo-sin-qt.md)).
`jase-cli` usa `jase-core` sin features.

## Hilos

- `update` y `view` de Iced: solo cambian el estado y lo dibujan. Nada de red, DNS, lectura
  de carpetas de juegos, extracción de imágenes ni archivos grandes (la config, pequeña, sí
  se guarda ahí).
- Red: tareas de tokio (UDP y HTTP) que vuelven como `Message` por un `Task` o una
  `Subscription` (`rust/jase/src/scan.rs`).
- Disco, descargas y búsquedas de exes: `spawn_blocking` (`app::blocking` en
  `rust/jase/src/app/mod.rs`), que nunca frenan el cierre: al cerrar la ventana el proceso
  sale sin esperarlas ([ADR 0003](adr/0003-hilos-daemon-y-relays.md)).
- Todo resultado tardío se descarta por generación (`ScanState` en `rust/jase/src/scan.rs`)
  o por número de pedido (`background::Request`, `country_request`, …): `update` ignora el
  mensaje que no coincide.

## Datos remotos (no confiables)

- Todo lo que manda un servidor o un master se valida en el parser: largos, rangos
  (`protocols::bounded_int`), forma. Sin `unwrap` ni pánicos sobre datos externos; un
  servidor raro se descarta, nunca tumba el escaneo (`rust/jase-core/tests/robustness.rs`
  con `proptest`).
- Texto remoto en la UI: siempre texto plano (Iced no interpreta marcado; los colores `^`
  se parten en tramos sin interpretar nada más, `rust/jase/src/colored.rs`). En los globos de
  la bandeja, sin caracteres de control y escapado donde el sistema lee marcado
  (`tray::plain_notice`, `tray::escape_markup`).
- Lo que llega a la línea de comandos del juego se valida (`launcher::is_mod_name`,
  `launcher::is_wad_name`) y se lanza con `std::process::Command` (argv, nunca un shell).
- La clave de Steam solo viaja a `api.steampowered.com` (`rust/jase-core/src/protocols/steamweb.rs`), siempre por HTTPS: con el proxy de Ajustes → Red va dentro del túnel `CONNECT`, que el proxy no lee.
- Todo agente HTTP sale de `net::agent_builder()` (`rust/jase-core/src/net.rs`), que pone el User-Agent y el proxy elegido.
- Enlaces `jase://`: análisis estricto (`jase_url::parse`) y confirmación del usuario.

## Configuración

- Cada campo se carga por separado con su valor por defecto; una config ilegible se
  copia a `.bad` antes de usar los valores por defecto.
- No se reescriben rutas guardadas por el usuario.

## Prohibido

- Índices de columna sueltos en la UI: siempre `enum Column` de `rust/jase/src/table/model.rs`.
- Textos visibles sin `tr`/`tr_fmt`/`tr_n` o sin traducción en los seis idiomas (lo vigila
  `rust/jase/tests/i18n_literals.rs`).
- Colores fuera de `Palette` (`rust/jase/src/theme/palette.rs`, un campo por color en
  todos los temas: si falta uno, no compila).
- Un juego nuevo sin sondeo real de su master ([ADR 0005](adr/0005-juegos-con-sondeo-real.md)).
- Versionar archivos de los juegos.

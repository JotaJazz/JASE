# Memoria del proyecto

Solo se agrega arriba; nunca se reescribe lo anterior. Cada etapa: 3 viñetas como mucho
(plantilla en [DEVELOPMENT_GUIDE](DEVELOPMENT_GUIDE.md)). No es un changelog: el detalle
está en git y en `packaging/release-notes.md`.

## 2026-11-17 — port a Rust, fase 4a (datos)
- `jase-core` lee y escribe config, favoritos, notas, grupos, recientes, listas guardadas, historial y la exportación de favoritos en el formato de Python 1.3.0; se probó con goldens de Python (mismos bytes al reguardar) y Python relee la config que escribe Rust. Suma amigos, alertas de partida, búsqueda de jugadores y enlaces `jase://`.
- Descartado: imitar el descarte de claves desconocidas de la config; Rust las conserva para que 1.3 y 2.0 convivan ([divergencias](../hoja-de-ruta/port-rust.md)).
- Abierto: fase 4b (launcher, detección de juegos, LAN, RCON); `passwords.py` va en la 5.

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

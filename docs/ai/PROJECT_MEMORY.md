# Memoria del proyecto

Solo se agrega arriba; nunca se reescribe lo anterior. Cada etapa: 3 viñetas como mucho
(plantilla en [DEVELOPMENT_GUIDE](DEVELOPMENT_GUIDE.md)). No es un changelog: el detalle
está en git y en `packaging/release-notes.md`.

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

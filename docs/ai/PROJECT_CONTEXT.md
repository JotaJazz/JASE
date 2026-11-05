# Contexto del proyecto

**JASE (Jimmy's All Seeing Eye):** navegador de servidores de juegos multijugador clásicos
al estilo del All-Seeing Eye: lista servidores por master con nombre, mapa, jugadores y
ping, y lanza el juego conectado con un doble clic.

## Objetivo
Una herramienta pequeña, rápida y fiable para jugadores de shooters clásicos en Windows
(con Linux como plataforma secundaria), que funcione con los masters que siguen vivos.

## No objetivos
- No es un servidor ni un master, ni aloja partidas.
- No descarga mapas ni mods del juego (solo imágenes de mapas, opcional).
- No soporta juegos sin un master vivo comprobado (ADR 0005).
- No usa CD keys reales ni evita protecciones de los juegos.

## Principios
- Lo que manda un servidor es no confiable: se valida y se muestra como texto plano.
- La UI nunca se congela: red y disco fuera del hilo de la UI.
- Configuración tolerante: nada de lo guardado se pierde por un campo dañado.
- Un juego entra con un sondeo real y capturas en los tests.
- Interfaz en seis idiomas (es, en, pt, fr, de, it); la clave de cada texto es el español.

## Stack y restricciones
Python ≥ 3.11 (CI: 3.11 y 3.13). Núcleo solo con la biblioteca estándar; app con PySide6,
keyring y Pillow (extra `gui`). Paquetes: instalador Inno Setup y zip portátil para
Windows, AppImage para Linux. Licencia MIT. Repo público: github.com/JotaJazz/JASE.

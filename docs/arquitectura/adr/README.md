# Decisiones de arquitectura (ADR)

Un ADR aceptado no se edita: si cambia la decisión, se escribe uno nuevo y el viejo pasa a
«Reemplazado por NNNN» (o «Reemplazado: no aplica», si el cambio de plataforma la vuelve
innecesaria). En la 2.0.0 (paso a Rust) se actualizaron 0001 y 0003, con una línea que lo
dice, y 0004–0006 solo en sus rutas. Los 0001–0006 son retroactivos: documentan decisiones ya tomadas,
con la fecha del commit que las introdujo.

| N.º | Decisión | Estado |
|---|---|---|
| [0001](0001-nucleo-sin-qt.md) | Núcleo sin UI (`jase-core`); la app es otro crate | Aceptado (actualizado en 2.0.0) |
| [0002](0002-gc-en-el-hilo-de-ui.md) | Recolección de basura solo en el hilo de la UI | Reemplazado: no aplica en Rust |
| [0003](0003-hilos-daemon-y-relays.md) | Trabajo lento fuera del hilo de la UI, sin frenar el cierre | Aceptado (actualizado en 2.0.0) |
| [0004](0004-masters-oficiales-y-personales.md) | Lista oficial de masters remota + empaquetada; gana la más nueva | Aceptado |
| [0005](0005-juegos-con-sondeo-real.md) | Un juego entra solo con un sondeo real de su master | Aceptado |
| [0006](0006-listas-guardadas.md) | Listas de servidores guardadas entre sesiones | Aceptado |

## Plantilla

```markdown
# NNNN. Título corto

- Estado: Propuesto | Aceptado | Reemplazado por NNNN | Reemplazado: no aplica
- Fecha: AAAA-MM-DD

## Contexto
Qué problema o fuerza obliga a decidir (2–4 líneas).

## Decisión
Qué se hace, en imperativo.

## Consecuencias
Qué se gana, qué se pierde, qué queda prohibido o obligado.
```

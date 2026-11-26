# Decisiones sobre la especificación v1.0

Ajustes acordados sobre `Argus-Reviewer-Arquitectura-y-Plan-Agente.docx`. Si este archivo contradice el .docx,
manda este archivo.

## D1 — Los módulos grandes nacen divididos
El presupuesto de 350 líneas por `.rs` es incompatible con un único `parser/javascript.rs` o `lexer/python.rs`.
Cada lexer y parser es un directorio con submódulos (`lexer/python/{mod,strings,numbers,punct}.rs`,
`parser/javascript/{expr,stmt,types,jsx}.rs`, ...). La sección 03 se lee con esa granularidad.

## D2 — Clones: hash exacto en v1, similitud después
BLAKE3 sobre el subárbol normalizado solo detecta clones Tipo-1 y Tipo-2 (idénticos tras renombrar). Un
porcentaje de similitud (`similarity_threshold`, "duplica al 98%") exige otra técnica (winnowing o MinHash sobre
k-gramas de nodos). En la Fase 4 se implementa el hash exacto; la similitud aproximada queda como ampliación de
esa fase y `similarity_threshold` se ignora hasta entonces.

## D3 — Contrato de diagnósticos desde el principio
La integración con harness de agentes es el objetivo del proyecto. El modelo `Diagnostic` y la salida
`--format agent` se definen pronto y crecen con cada fase, en lugar de llegar en la Fase 6. Así `argus scan`
es usable desde un harness con las reglas que ya existan.

## D4 — Criterio realista de la Fase 1
"Round-trip byte a byte" lo garantiza el diseño (tokens contiguos + CST que conserva trivia), no la cobertura de
la gramática. La Fase 1 se acepta cuando:
- el round-trip pasa sobre fixtures y sobre la stdlib de Python completa (y un corpus TS real);
- el lexer no produce errores sobre código válido de esos corpus;
- el parser cubre la gramática común y marca con nodos `Error` lo que no reconoce, sin entrar en pánico ni bucles.

## D5 — Refactors destructivos en `--dry-run` por defecto
`argus clean` y `argus split` muestran el diff (`similar`) y solo escriben con `--apply`.

## D6 — Categoría extra de comentario: `Marker`
Además de Doc, Directive, License y Narrative, los comentarios `TODO`/`FIXME`/`HACK`/`XXX`/`NOTE` se clasifican
como `Marker`. `argus clean` nunca los borra: son deuda registrada, no ruido.

## D7 — Toolchain GNU en Windows
La máquina de desarrollo no tiene MSVC Build Tools; se usa `x86_64-pc-windows-gnu`. Ninguna decisión del
código puede depender del target.

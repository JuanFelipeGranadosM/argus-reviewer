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

## D8 — Las exclusiones del usuario se suman a las básicas
`node_modules`, `venv`, `dist`, `build`, `__pycache__`, `target` y las entradas ocultas se excluyen siempre.
`[general].exclude` añade patrones en lugar de reemplazar la lista. Si se reemplazara, un `.argus.toml` con
`exclude = ["pkg"]` haría que se analizara `node_modules`, y en un harness eso es una trampa silenciosa.

## D9 — `comment-soup` mide comentarios cortos y aislados, no todos los narrativos
Contar todos los comentarios narrativos marcaba el 10 % de la stdlib de CPython (347 archivos), escrita por
personas. Solo cuentan los comentarios narrativos de 6 palabras o menos que no forman bloque con la línea
anterior o la siguiente, ya sean comentarios en línea propia (explicaciones del porqué) o al final de línea
(anotaciones de tablas). Con eso, la stdlib baja a 6 archivos, todos tablas generadas. La sopa ligera (ratio
bajo 0,25) no se marca: decidir qué comentario sobra es tarea de `argus clean` (Fase 5).

## D10 — Identificadores Unicode como superconjunto
Sin tablas Unicode (`unicode-ident` no está entre las dependencias permitidas), el lexer trata como parte de un
identificador cualquier carácter no ASCII que no sea espacio. El código Python válido nunca da error (marcas
combinantes, selectores de variante, `℘`, letras matemáticas). A cambio, un identificador inválido como `€` no
se detecta en el lexer: argus no es un compilador y prioriza no dar falsos positivos sobre código válido.

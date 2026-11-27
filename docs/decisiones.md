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

## D11 — Lexer JS/TS con heurísticas de contexto, sin parser
Sin gramática no se sabe si `/` abre una regex ni si `<` abre JSX. Se decide por el token significativo
anterior, como los escáneres clásicos de JS:
- `/` abre una regex salvo tras algo que cierra una expresión (identificador, literal, `)`, `]`, `}`, `++`,
  `this`, ...). Excepciones: una palabra reservada tras `.` es un nombre de propiedad (`x.default / 2`); la `)`
  de `if`/`while`/`for`/`with` abre una sentencia (`if (x) /re/.test(y)`); un `!` tras una expresión es la
  aserción no nula de TypeScript (`alto! / 12`); `yield` y `await` van seguidos de una expresión.
- `<` abre JSX solo donde cabe una expresión y fuera de `.ts`/`.mts`/`.cts`. En `.tsx` se reconocen las listas
  de parámetros de tipo (`<T,>`, `<T extends U>`, `<T = V>`) para no confundirlas con etiquetas.
- Los `>` se emiten sueltos: en `Array<Array<T>>` cada uno cierra un nivel de genéricos, y el parser recompone
  `>>`, `>=` o `>>>=` cuando no hay trivia entre ellos.
- Las plantillas se emiten por tramos (`TemplateHead`/`Middle`/`Tail`) y el código de sus `${...}` se lexea
  como código normal, con una pila de contextos que también gobierna las llaves de JSX.

Queda ambigua la `}` de bloque seguida de `/` (`}\n/re/.test(x)`): se trata como división, que es lo habitual
tras un objeto literal. Validado con `scripts/oracle_typescript.mjs` (comentarios, regex, plantillas y JsxText
contra el parser de TypeScript) sobre GO-Asset y su `node_modules`: 24.119 archivos con 0 discrepancias, y
round-trip sin errores sobre 49.100 archivos (624 MB, incluidos los worktrees).

## D12 — Los archivos minificados o generados no se auditan
Un bundle copiado a `assets/` (p. ej. por Capacitor) no lo escribió una persona ni un agente; marcarlo como
`file-too-long` es ruido. Si un archivo de 4 KiB o más tiene una media de más de 300 bytes por línea (el código
escrito a mano ronda los 30-40), se omite y se cuenta en `summary.files_generated`. En la stdlib de CPython solo
omite dos tablas generadas de `site-packages`.

## D13 — Esqueleto estructural antes que la gramática completa
El objetivo de Argus es que el agente no tenga que leer el proyecto: reciba diagnósticos compactos y no
gaste ventana de contexto ni cuota. Por eso las reglas por función no esperan al parser completo. Una primera
capa (`src/structure/`) reconoce sobre los tokens las funciones (nombre, parámetros, anotaciones, cuerpo,
complejidad), los bloques que capturan errores, `eval`/`exec`/`new Function` y el `any` de TypeScript. El
parser CST la sustituirá por dentro sin cambiar las reglas. El reconocimiento es conservador: ante una forma
que no entiende, no inventa una función.
- **Longitud de función en líneas propias.** Las líneas interiores de las funciones anidadas no cuentan para
  la que las contiene. Contar todo marcaba cada `describe(...)` de los tests de GO-Asset (100 hallazgos, 26
  de ellos callbacks de prueba). Con líneas propias quedan 45, todos funciones que conviene dividir.
- **Complejidad ciclomática**: 1 más cada `if`, bucle, `case`, `catch`/`except`, `&&`/`||`/`??` (también
  `and`/`or`) y ternario propios. Los de las funciones anidadas van a la anidada.
- **Captura vacía**: en JS/TS, `catch {}` y `.catch(() => {})` sin nada dentro; un comentario cuenta como
  explicación. En Python solo se marca el `except:` o `except Exception:` cuyo cuerpo es `pass`/`...` sin
  comentario: ignorar una excepción concreta (`except ImportError: pass`) es una decisión, no un descuido.
- **`any` explícito** solo en posición de tipo (`: any`, `as any`, `<any>`, `| any`, `= any`, `=> any`); un
  nombre `any` no se marca. Un diagnóstico por archivo con todas las líneas, para no inflar el JSON.
- **Anotaciones de Python** (aviso, un diagnóstico por archivo): solo funciones públicas no anidadas, sin
  `self`/`cls`, sin las pruebas `test*` y sin exigir retorno a `__init__`. La stdlib de CPython no sirve de
  referencia de falsos positivos para esta regla: no se anota por diseño (los tipos están en typeshed).

## D14 — Modo incremental: `argus diff` lista solo lo que el cambio introduce o empeora
Auditar el proyecto entero en cada turno devuelve al agente cientos de problemas que no causó y hace crecer
el informe con el repositorio. `argus diff [ruta] [--base REV]` analiza solo los archivos distintos desde el
ancestro común de `REV` (por defecto `HEAD`) y `HEAD`: el árbol de trabajo, el índice y los archivos nuevos
sin seguimiento que no ignora `.gitignore`. Con `--base main` cubre todos los commits de la rama.
- **Base**: cada archivo cambiado se analiza también en su versión base (`git cat-file --batch`, un solo
  proceso para todos), con la misma ruta y configuración. Las líneas tocadas salen de `git diff -U0 -M`.
- **Emparejamiento**: por regla y mensaje con los números sustituidos (las líneas se desplazan); las reglas
  de un diagnóstico por archivo (BLOAT-001, SOUP-001, TYPE-002, TYPE-003) solo por regla.
- **Nuevo o empeorado**: un diagnóstico fuera de las líneas tocadas siempre es previo. Uno tocado es nuevo
  salvo que tenga pareja en la base con una métrica (líneas, parámetros, complejidad, usos) igual o mayor:
  alargar una función que ya era larga se informa; tocarla sin empeorarla, no.
- **Resumen**: `summary.incremental` (`base`, `base_commit`, `files_changed`, `preexisting_issues`). Los
  previos se cuentan pero no se listan. Fuera de un repositorio git es un fallo de la herramienta (código 2).
- **Coste**: en GO-Asset, 83 archivos cambiados frente a `HEAD` dan 39 diagnósticos (21 KB) en lugar de los
  del proyecto entero, en 0,75 s con el binario de depuración (225 ms de ellos son de git).

## D15 — Tres niveles: contratos, avisos y sugerencias, y cada proyecto ajusta los suyos
No todas las formas de programar son iguales, y las guías se contradicen entre sí: la de Google para
TypeScript prohíbe `export default` y los campos `#private`, y Next.js exige `export default` en sus páginas.
PEP 8 admite `mixedCase` donde ya domina y dice que la consistencia dentro de un proyecto (y aún más dentro
de un módulo) importa más que la guía. Un arnés que impusiera una sola escuela marcaría en masa código
correcto. Por eso cada regla está en uno de tres niveles:
- **Contrato (`error`, bloquea)**: defectos objetivos en los que coinciden PEP 8, el manual de TypeScript y
  la guía de Google: presupuestos de tamaño y complejidad, capturas vacías, `eval`, `any`, `debugger`.
- **Aviso (`warning`)**: casi siempre un error, aunque a veces es deliberado: `== None`, `==` laxo (salvo
  `== null`, que las guías admiten para `null` y `undefined` a la vez), `var`, funciones públicas sin anotar.
- **Sugerencia (`suggestion`, nunca bloquea)**: cuestiones de estilo con varias escuelas válidas. No imponen
  una convención: piden la del propio código. `ARGUS-STYLE-001` deduce la convención de nombres de cada
  archivo (al menos 5 funciones y un 80 % en `snake_case` o `camelCase`) y señala las que se desvían, que
  es justo el error típico de un agente: escribir en su estilo por defecto y no en el del proyecto. Una sola
  palabra, `PascalCase` (clases, componentes de React) y nombres de framework (`setUp`) no cuentan.

Cada equipo ajusta los niveles en `.argus.toml`: `[rules]` apaga, suaviza o endurece cualquier regla por su
ID (un ID inexistente es un error de configuración), y `[style]` fija la convención de nombres del proyecto
(`snake_case`, `camelCase`, `auto` u `off`), que entonces se exige en todos los archivos, también en los
nuevos.

En `--format agent` las sugerencias llegan con `severity: "suggestion"` y se cuentan en
`summary.suggestions`; `warnings` cuenta solo los avisos. Validación:
- Stdlib de CPython (3544 archivos): `== None` en 27 archivos, casi todos paquetes de terceros. `lambda`
  asignada en 56. STYLE-001 en 73 (un 2 %), todos mezclas reales, como los alias `camelCase` de
  `threading` o de `bs4`.
- GO-Asset: ningún hallazgo nuevo; es TypeScript estricto y consistente.

## D16 — Sugerencias que traen la solución escrita: bucles acumuladores
Una sugerencia que solo dice "esto se podría escribir mejor" obliga al agente a releer el código y a
decidir, que es lo que Argus quiere ahorrarle. `ARGUS-STYLE-002` reconoce el bucle que solo llena una
colección recién creada y entrega la expresión equivalente ya escrita, con las líneas que sustituye:
`r = [x * 2 for x in xs]` (también conjuntos, diccionarios y la copia `list(xs)`) o
`const r = xs.filter((x) => x).map((x) => f(x));`. El agente la aplica en un turno.

Solo se sugiere cuando la reescritura conserva el comportamiento:
- La colección se crea vacía (`[]`, `{}`, `set()`, `list()`, `dict()`) justo antes del bucle, y el cuerpo
  es una única inserción (`append`, `add`, `d[k] = v`, `push` de un solo argumento), como mucho bajo un
  `if` sin `else`. No puede haber `continue` ni más sentencias, ni un `for ... else`.
- Nada de lo que se evalúa nombra la colección (`r.append(len(r))`) ni usa `await` o `yield`.
- Python: la variable del bucle no se lee después (sobrevive a un `for` pero no a una comprensión), ni la
  condición ni el valor usan `locals()`, `vars()` o `super()`, y el bucle no está en el cuerpo de una
  clase, donde una comprensión no ve los atributos.
- El resultado cabe en una línea: 88 caracteres en Python, 100 en JS/TS. Si no cabe, el bucle es más claro.

Si el bucle tenía comentarios, la instrucción pide conservarlos. En JS/TS, si no se sabe que el origen es un
array (`Object.entries(...)`, `Array.from(...)`), avisa de que `map` y `filter` necesitan uno. Validación:
- Stdlib de CPython: 128 sugerencias en 84 archivos; todas las revisadas son reescrituras correctas.
- GO-Asset: ninguna, y es correcto. Sus bucles con `[]` previo usan `continue`, varias sentencias o
  `await`, o leen la colección.

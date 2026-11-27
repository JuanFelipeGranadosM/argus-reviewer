# argus-reviewer

Motor determinista en Rust para auditar el código Python y JS/TS que generan los agentes (Claude Code, Cursor,
...). Detecta la "entropía agéntica" sin volver a llamar a ningún modelo de lenguaje.

Especificación: [`docs/Argus-Reviewer-Arquitectura-y-Plan-Agente.docx`](docs/Argus-Reviewer-Arquitectura-y-Plan-Agente.docx)
· Ajustes: [`docs/decisiones.md`](docs/decisiones.md) · Reglas para agentes que trabajan en el repo: [`CLAUDE.md`](CLAUDE.md)

## Estado

Fase 1 en curso. Lo que funciona hoy:

- Lexer de Python sin pérdida, validado con round-trip byte a byte y contra `tokenize` de CPython sobre la stdlib.
- Lexer de JavaScript/TypeScript/JSX sin pérdida, validado con round-trip y contra el parser de TypeScript
  (comentarios, regex, plantillas y texto JSX) sobre un monorepo real y su `node_modules`.
- Esqueleto estructural (funciones, parámetros, anotaciones, complejidad, capturas de errores) para Python y
  JS/TS, primera capa del parser (D13).
- `argus scan` con reglas de presupuestos, tipado y seguridad para Python y JS/TS. Los archivos minificados o generados (media de
  más de 300 bytes por línea) se omiten y se cuentan en `files_generated`.
- `argus diff`, modo incremental: solo los problemas que introduce o empeora el cambio frente a git (D14).
- Tres niveles de reglas (contrato, aviso, sugerencia) ajustables por proyecto con `[rules]` y `[style]` (D15).
- Sugerencias con la solución escrita: un bucle que solo llena una colección llega con su comprensión o su
  `map`/`filter` equivalente, comprobada para que no cambie el comportamiento (D16).

## Uso

```sh
cargo build
argus scan [ruta] [--format human|agent] [--config ruta/.argus.toml]
argus diff [ruta] [--base HEAD|main|<rev>] [--format human|agent] [--config ruta/.argus.toml]
```

`argus diff` audita solo los archivos que cambiaron desde el ancestro común de `--base` y `HEAD` (árbol de
trabajo, índice y archivos nuevos no ignorados) y lista los problemas que el cambio introduce o empeora: una
función que ya era larga solo aparece si se alarga. Los que ya existían se cuentan en
`summary.incremental.preexisting_issues`. Requiere `git` en el `PATH`.

| Código de salida | Significado |
|---|---|
| 0 | Sin errores bloqueantes (puede haber avisos) |
| 1 | Hay errores bloqueantes |
| 2 | Fallo de la herramienta: ruta inexistente, configuración inválida, `diff` fuera de un repositorio git, ... |

La configuración se lee de `<ruta>/.argus.toml` si existe (esquema en la sección 5.2 de la especificación).
`node_modules`, `venv`, `dist`, `build`, `__pycache__`, `target` y las entradas ocultas se excluyen siempre;
`[general].exclude` añade patrones: `nombre`, `*.sufijo` o `ruta/relativa`.

Cada proyecto adapta las reglas a su forma de programar:

```toml
[rules]                       # nivel por ID: "off" | "suggestion" | "warning" | "error"
"ARGUS-IDIOM-002" = "off"     # este proyecto acepta `==`
"ARGUS-STYLE-001" = "warning" # y quiere que la consistencia de nombres avise

[style]                       # convención de nombres de funciones: "auto" (la de cada archivo),
js_function_naming = "camelCase"      # "snake_case", "camelCase" u "off"
python_function_naming = "snake_case"
```

## Reglas

| ID | Regla | Severidad | Detecta |
|---|---|---|---|
| `ARGUS-SYNTAX-001` | `syntax-error` | error | Error léxico (cadena sin cerrar, desindentación inconsistente, ...) |
| `ARGUS-BLOAT-001` | `file-too-long` | error | Archivo por encima de `budgets.max_file_lines` |
| `ARGUS-BLOAT-002` | `function-too-long` | error | Función con más líneas propias (sin contar funciones anidadas) que `budgets.max_function_lines` |
| `ARGUS-BLOAT-003` | `too-many-parameters` | error | Más parámetros que `budgets.max_parameters` (sin `self`/`cls`) |
| `ARGUS-BLOAT-004` | `high-complexity` | error | Complejidad ciclomática por encima de `budgets.max_cyclomatic_complexity` |
| `ARGUS-SOUP-001` | `comment-soup` | aviso | Exceso de comentarios narrativos cortos y aislados frente a `budgets.max_comment_to_code_ratio` |
| `ARGUS-TYPE-001` | `bare-suppression` | aviso | Python: `# type: ignore`, `# pyright: ignore` o `# noqa` sin código. JS/TS: `@ts-ignore`, `@ts-nocheck` y `eslint-disable` sin regla |
| `ARGUS-TYPE-002` | `explicit-any` | error | `any` en posición de tipo en TypeScript (`typing.forbid_ts_explicit_any`) |
| `ARGUS-TYPE-003` | `missing-annotations` | aviso | Funciones públicas de Python sin anotar parámetros o retorno (`typing.require_python_*`) |
| `ARGUS-SEC-001` | `empty-catch` | error | `catch {}`, `.catch(() => {})` o `except Exception: pass` sin comentario (`security.forbid_empty_catch`) |
| `ARGUS-SEC-002` | `dynamic-eval` | error | `eval`, `exec` o `new Function` (`security.forbid_dynamic_eval`) |
| `ARGUS-IDIOM-001` | `none-equality` | aviso | Python: `== None` o `!= None` en lugar de `is None` (PEP 8) |
| `ARGUS-IDIOM-002` | `loose-equality` | aviso | JS/TS: `==` o `!=` salvo frente a `null`/`undefined` |
| `ARGUS-IDIOM-003` | `var-declaration` | aviso | JS/TS: `var` (fuera de `declare var` y de los `.d.ts`) |
| `ARGUS-IDIOM-004` | `debugger-statement` | error | JS/TS: sentencia `debugger` olvidada |
| `ARGUS-IDIOM-005` | `lambda-assignment` | sugerencia | Python: `nombre = lambda ...` en lugar de `def` (PEP 8) |
| `ARGUS-STYLE-001` | `naming-inconsistency` | sugerencia | Función cuyo nombre no sigue la convención del archivo o la fijada en `[style]` |
| `ARGUS-STYLE-002` | `accumulator-loop` | sugerencia | Bucle que solo llena una colección; trae escrita la comprensión o el `map`/`filter` equivalente (D16) |
| `ARGUS-IO-001` | `unreadable-file` | aviso | Archivo que no es UTF-8 o supera 4 GiB |

Los IDs son estables: una regla publicada nunca cambia de ID. Los errores son contratos objetivos y bloquean;
los avisos señalan problemas probables; las sugerencias son de estilo, piden la convención del propio
proyecto y nunca bloquean (D15).

## Integración en harness y CI

`--format agent` emite un JSON determinista (mismo código y misma configuración producen los mismos bytes)
con instrucciones de remediación para que el agente corrija en un solo turno:

```json
{
  "summary": { "files_scanned": 7, "files_unsupported": 0, "files_generated": 1, "blocking_issues": 2, "warnings": 4, "suggestions": 1 },
  "diagnostics": [
    {
      "id": "ARGUS-TYPE-001",
      "severity": "warning",
      "rule": "bare-suppression",
      "file": "bare.py",
      "line_start": 1,
      "line_end": 1,
      "message": "Supresión sin código: `# type: ignore` silencia cualquier error de la línea.",
      "agent_action": {
        "type": "add_suppression_code",
        "lines": [1],
        "instruction": "Usar `# type: ignore[código]` con el código concreto, o corregir el error y quitar la supresión."
      }
    }
  ]
}
```

Este JSON es un contrato: los campos solo se añaden, nunca se renombran ni cambian de tipo. Las rutas son
relativas a la raíz analizada y usan `/`. El servidor MCP (`argus mcp`) llegará en la Fase 6; mientras tanto, el
harness puede ejecutar `argus diff --format agent` al final de cada turno del agente (o `--base main` antes
de abrir un PR) y entregarle la salida. En modo incremental el resumen añade:

```json
"incremental": { "base": "HEAD", "base_commit": "0fbec54…", "files_changed": 3, "preexisting_issues": 5 }
```

## Desarrollo

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

CI (GitHub Actions) ejecuta lo mismo en Linux y Windows, y repite el round-trip y los oráculos contra la stdlib
de CPython y contra un corpus de paquetes npm fijados:

```sh
ARGUS_JS_ROUNDTRIP_DIR=<carpeta> cargo test --test js_lexer_tests -- --ignored --nocapture
node scripts/oracle_typescript.mjs <node_modules/typescript/lib/typescript.js> target/debug/argus <carpeta>
```

# argus-reviewer

Motor determinista en Rust para auditar el código Python y JS/TS que generan los agentes (Claude Code, Cursor,
...). Detecta la "entropía agéntica" sin volver a llamar a ningún modelo de lenguaje.

Especificación: [`docs/Argus-Reviewer-Arquitectura-y-Plan-Agente.docx`](docs/Argus-Reviewer-Arquitectura-y-Plan-Agente.docx)
· Ajustes: [`docs/decisiones.md`](docs/decisiones.md) · Reglas para agentes que trabajan en el repo: [`CLAUDE.md`](CLAUDE.md)

## Estado

Fase 1 en curso. Lo que funciona hoy:

- Lexer de Python sin pérdida, validado con round-trip byte a byte y contra `tokenize` de CPython sobre la stdlib.
- `argus scan` con reglas basadas en tokens, solo para Python. Los archivos JS/TS se cuentan en
  `files_unsupported`.

## Uso

```sh
cargo build
argus scan [ruta] [--format human|agent] [--config ruta/.argus.toml]
```

| Código de salida | Significado |
|---|---|
| 0 | Sin errores bloqueantes (puede haber avisos) |
| 1 | Hay errores bloqueantes |
| 2 | Fallo de la herramienta: ruta inexistente, configuración inválida, ... |

La configuración se lee de `<ruta>/.argus.toml` si existe (esquema en la sección 5.2 de la especificación).
`node_modules`, `venv`, `dist`, `build`, `__pycache__`, `target` y las entradas ocultas se excluyen siempre;
`[general].exclude` añade patrones: `nombre`, `*.sufijo` o `ruta/relativa`.

## Reglas

| ID | Regla | Severidad | Detecta |
|---|---|---|---|
| `ARGUS-SYNTAX-001` | `syntax-error` | error | Error léxico (cadena sin cerrar, desindentación inconsistente, ...) |
| `ARGUS-BLOAT-001` | `file-too-long` | error | Archivo por encima de `budgets.max_file_lines` |
| `ARGUS-SOUP-001` | `comment-soup` | aviso | Exceso de comentarios narrativos cortos y aislados frente a `budgets.max_comment_to_code_ratio` |
| `ARGUS-TYPE-001` | `bare-suppression` | aviso | `# type: ignore`, `# pyright: ignore` o `# noqa` sin código concreto |
| `ARGUS-IO-001` | `unreadable-file` | aviso | Archivo que no es UTF-8 o supera 4 GiB |

Los IDs son estables: una regla publicada nunca cambia de ID.

## Integración en harness y CI

`--format agent` emite un JSON determinista (mismo código y misma configuración producen los mismos bytes)
con instrucciones de remediación para que el agente corrija en un solo turno:

```json
{
  "summary": { "files_scanned": 5, "files_unsupported": 1, "blocking_issues": 2, "warnings": 3 },
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
harness puede ejecutar `argus scan --format agent` y entregar la salida al agente.

## Desarrollo

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

CI (GitHub Actions) ejecuta lo mismo en Linux y Windows, y repite el round-trip y el oráculo contra la stdlib
de CPython.

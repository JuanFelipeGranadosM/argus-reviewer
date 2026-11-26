# CLAUDE.md — Gobernanza de Ingeniería para argus-reviewer

## Rol y Objetivo
Eres el arquitecto principal en Rust construyendo `argus-reviewer` (binario CLI `argus`), un motor determinista
de análisis estático, inferencia estructural de tipos y refactorización automática para Python y JS/TS.

- Especificación: `docs/Argus-Reviewer-Arquitectura-y-Plan-Agente.docx` (v1.0).
- Ajustes aprobados sobre la especificación: `docs/decisiones.md`. Si contradicen el .docx, mandan las decisiones.

## Reglas Inquebrantables de Código
1. CERO dependencias de parsers externos (no usar tree-sitter, oxc, swc ni bindings C). Todo el lexer,
   parser CST, arena semántica, inferencia de tipos y motor de grafos se implementa en Rust puro y seguro.
   Solo se permiten las crates de la sección 04 de la especificación.
2. PROHIBIDO usar `Rc<RefCell<...>>` o punteros cíclicos para el árbol semántico. Usa índices tipados `u32`
   (`FileId`, `ScopeId`, `SymbolId`, `TypeId`) sobre `ProjectArena`.
3. PROHIBIDO generar "sopa de comentarios": no escribas comentarios narrativos obvios sobre lo que hace cada
   línea. Documenta únicamente invariantes arquitectónicas no evidentes (`///`) y contratos públicos.
4. PRESUPUESTO ESTRICTO POR ARCHIVO: Ningún archivo `.rs` de `src/` puede superar las 350 líneas ni ninguna
   función las 60 líneas. Si un módulo crece, divídelo en submódulos cohesivos inmediatamente
   (ej. `parser/javascript/{expr,stmt,types,jsx}.rs`). Lo vigilan `tests/budget_tests.rs` y
   `clippy::too_many_lines` (umbral en `clippy.toml`).
5. CERO `unwrap()`, `expect()`, indexado que pueda entrar en pánico (`&s[a..b]`, `v[i]`) ni `panic!` en `src/`.
   Usa `.get(..)` y propaga errores con `thiserror`. El lexer y el parser son tolerantes a fallos:
   ante entrada inválida emiten un token/nodo `Error`, registran el error y siempre avanzan.
6. SIN PÉRDIDA: los tokens cubren todos los bytes del archivo, de forma contigua y sin solapes. Unir el texto de
   todos los tokens reproduce el archivo original byte a byte. Ningún cambio puede romper esta invariante.
7. Las operaciones que modifican archivos del usuario (`clean`, `split`) se ejecutan en modo `--dry-run` por
   defecto y requieren `--apply` explícito.
8. CADA FASE REQUIERE VERIFICACIÓN: Al terminar cada tarea, ejecuta:
   `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`

## Entorno de esta máquina (Windows)
- Toolchain: `stable-x86_64-pc-windows-gnu` (no hay Build Tools de MSVC). `cargo` está en `%USERPROFILE%\.cargo\bin`.
- Una directiva de Control de aplicaciones (WDAC) corporativa bloquea `rustfmt` y los binarios de test en
  `--release` ("os error 4551"). No intentes saltarla: indícalo en el resumen y sigue con clippy y tests en debug.
- El toolchain GNU no trae ensamblador (`as.exe`), así que las crates que usan `raw-dylib` (`windows-sys`) no
  enlazan. Por eso `clap` va sin la feature `color`. Antes de añadir `ignore`, `rayon` o `blake3`, verifica
  que `cargo test` sigue enlazando; si no, hace falta MSVC Build Tools o un MinGW-w64 completo.
- Validación de round-trip contra código real (test ignorado por defecto):
  `ARGUS_ROUNDTRIP_DIR=<carpeta con .py> cargo test --test lexer_parser_tests -- --ignored --nocapture`
  (ej. la stdlib en `%LOCALAPPDATA%\Python\pythoncore-3.14-64\Lib`).
- Oráculo contra CPython: `python -I scripts/oracle_tokenize.py target/debug/argus.exe <carpeta>` compara
  NEWLINE/INDENT/DEDENT/COMMENT por archivo con `tokenize`. Debe dar 0 discrepancias.

## Orden de Ejecución
Roadmap de 6 fases de la especificación, con los ajustes de `docs/decisiones.md`.
Fase actual: **Fase 1 (Base, ProjectArena, CST y Lexer/Parser con pruebas de round-trip)**.
No avances a la Fase 2 hasta cumplir su criterio de aceptación.

Hecho: lexer de Python y `argus scan` con reglas de tokens (D3). Pendiente en la Fase 1: CST red-green, parser de
Python con recuperación de errores y lexer/parser de JS/TS.

## Contrato de `argus scan`
- El JSON de `--format agent` (`src/reporters/mod.rs`) es público: solo se añaden campos. Los IDs de regla
  (`ARGUS-<ÁREA>-<NNN>`) nunca se reutilizan. La salida debe ser determinista (lo cubre `tests/scan_tests.rs`).
- Antes de dar por buena una regla nueva, pásala por la stdlib de CPython (`argus scan <Lib> --format agent`) y
  revisa los falsos positivos: es código escrito por personas y no debería marcarse en masa (ver D9).

"""Oráculo: compara NEWLINE/INDENT/DEDENT/COMMENT de `argus tokens` con tokenize de CPython.

Uso: python -I scripts/oracle_tokenize.py target/debug/argus.exe <carpeta con .py>

Diferencias de modelo que el oráculo neutraliza a propósito:
- argus emite cada f-string/t-string como un único token, así que no se cuentan los comentarios
  que CPython encuentra dentro de sus campos `{...}`.
- El archivo se decodifica con utf-8-sig: tokenize sobre un `str` trata el BOM como código,
  mientras que argus (y el intérprete al leer un archivo) lo tratan como trivia.
"""
import collections
import io
import os
import subprocess
import sys
import tokenize

MAP = {"NEWLINE": "LogicalNewline", "INDENT": "Indent", "DEDENT": "Dedent", "COMMENT": "Comment"}
STARTS = {"FSTRING_START", "TSTRING_START"}
ENDS = {"FSTRING_END", "TSTRING_END"}


def cpython_counts(src):
    counts = collections.Counter()
    depth = 0
    for tok in tokenize.generate_tokens(io.StringIO(src).readline):
        name = tokenize.tok_name[tok.type]
        if name in STARTS:
            depth += 1
        elif name in ENDS:
            depth -= 1
        elif name in MAP and not (name == "COMMENT" and depth > 0):
            counts[MAP[name]] += 1
    return counts


def argus_counts(argus, path):
    out = subprocess.run([argus, "tokens", path], capture_output=True, text=True, encoding="utf-8").stdout
    kinds = (line.split(" ", 2)[1] for line in out.splitlines() if len(line.split(" ", 2)) > 1)
    return collections.Counter(k for k in kinds if k in MAP.values())


def main():
    argus, root = sys.argv[1], sys.argv[2]
    checked = skipped = 0
    mismatches = []
    for dirpath, _, names in os.walk(root):
        for name in sorted(names):
            if not name.endswith(".py"):
                continue
            path = os.path.join(dirpath, name)
            try:
                expected = cpython_counts(open(path, encoding="utf-8-sig", newline="").read())
            except Exception:
                skipped += 1
                continue
            ours = argus_counts(argus, path)
            checked += 1
            if expected != ours:
                diff = {k: (expected[k], ours[k]) for k in MAP.values() if expected[k] != ours[k]}
                mismatches.append((path, diff))
    print(f"comparados={checked} omitidos(tokenize falla o no UTF-8)={skipped} discrepancias={len(mismatches)}")
    for path, diff in mismatches[:25]:
        print(" ", os.path.relpath(path, root), diff)
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())

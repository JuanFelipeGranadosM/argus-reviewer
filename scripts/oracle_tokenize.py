"""Oráculo: compara NEWLINE/INDENT/DEDENT/COMMENT de `argus tokens` con tokenize de CPython.

Uso: python -I scripts/oracle_tokenize.py target/debug/argus.exe <carpeta con .py>
"""
import collections, io, os, subprocess, sys, tokenize
argus, root = sys.argv[1], sys.argv[2]
MAP = {"NEWLINE": "LogicalNewline", "INDENT": "Indent", "DEDENT": "Dedent", "COMMENT": "Comment"}
checked = skipped = 0
mismatches = []
for dirpath, _, names in os.walk(root):
    for name in names:
        if not name.endswith(".py"):
            continue
        path = os.path.join(dirpath, name)
        try:
            src = open(path, encoding="utf-8", newline="").read()
            py = collections.Counter(MAP[tokenize.tok_name[t.type]] for t in tokenize.generate_tokens(io.StringIO(src).readline) if tokenize.tok_name[t.type] in MAP)
        except Exception:
            skipped += 1
            continue
        out = subprocess.run([argus, "tokens", path], capture_output=True, text=True, encoding="utf-8").stdout
        ours = collections.Counter(line.split(" ", 2)[1] for line in out.splitlines() if line.split(" ", 2)[1:2] and line.split(" ", 2)[1] in MAP.values())
        checked += 1
        if py != ours:
            diff = {k: (py[k], ours[k]) for k in MAP.values() if py[k] != ours[k]}
            mismatches.append((path, diff))
print(f"comparados={checked} omitidos(tokenize falla o no UTF-8)={skipped} discrepancias={len(mismatches)}")
for path, diff in mismatches[:25]:
    print(" ", os.path.relpath(path, root), diff)
sys.exit(1 if mismatches else 0)

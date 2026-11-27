#!/usr/bin/env node
// Oráculo del lexer JS/TS de Argus contra el parser de TypeScript.
//
// Uso: node scripts/oracle_typescript.mjs <typescript.js> <argus.exe> <carpeta> [--limit N]
//
// El parser de TypeScript sabe, por la gramática, si `/` abre una regex y si `<` abre JSX; el lexer
// de Argus lo decide con heurísticas. Se comparan, en offsets de bytes UTF-8, los comentarios, las
// regex, los tramos de plantilla y los JsxText. Los archivos con errores de sintaxis para
// TypeScript se omiten. Sale con 1 si hay discrepancias.
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { extname, join, resolve } from "node:path";

const [tsPath, exe, root, ...rest] = process.argv.slice(2);
if (!tsPath || !exe || !root) {
  console.error("uso: oracle_typescript.mjs <typescript.js> <argus.exe> <carpeta> [--limit N]");
  process.exit(2);
}
const limitAt = rest.indexOf("--limit");
const limit = limitAt >= 0 ? Number(rest[limitAt + 1]) : Infinity;
const ts = createRequire(import.meta.url)(resolve(tsPath));

const KINDS = {
  ".ts": ts.ScriptKind.TS, ".mts": ts.ScriptKind.TS, ".cts": ts.ScriptKind.TS,
  ".tsx": ts.ScriptKind.TSX, ".jsx": ts.ScriptKind.JSX,
  ".js": ts.ScriptKind.JS, ".mjs": ts.ScriptKind.JS, ".cjs": ts.ScriptKind.JS,
};
const PIECES = new Map([
  [ts.SyntaxKind.RegularExpressionLiteral, "Regex"],
  [ts.SyntaxKind.NoSubstitutionTemplateLiteral, "TemplateNoSub"],
  [ts.SyntaxKind.TemplateHead, "TemplateHead"],
  [ts.SyntaxKind.TemplateMiddle, "TemplateMiddle"],
  [ts.SyntaxKind.TemplateTail, "TemplateTail"],
  [ts.SyntaxKind.JsxText, "JsxText"],
]);
const COMPARED = new Set(["Comment", ...PIECES.values()]);

function* walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(path);
    else if (entry.isFile() && extname(entry.name) in KINDS) yield path;
  }
}

/** Offset en bytes UTF-8 de cada unidad UTF-16 de `text`. */
function byteOffsets(text) {
  const map = new Uint32Array(text.length + 1);
  let bytes = 0;
  for (let i = 0; i < text.length; i++) {
    map[i] = bytes;
    const c = text.charCodeAt(i);
    if (c >= 0xd800 && c <= 0xdbff && i + 1 < text.length) {
      map[++i] = bytes;
      bytes += 4;
    } else bytes += c < 0x80 ? 1 : c < 0x800 ? 2 : 3;
  }
  map[text.length] = bytes;
  return map;
}

function expected(path, text) {
  const sf = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, KINDS[extname(path)]);
  if (sf.parseDiagnostics.length > 0) return null;
  const at = byteOffsets(text);
  const found = new Set();
  const comment = (r) => found.add(`Comment ${at[r.pos]}..${at[r.end]}`);
  const visit = (node) => {
    if (node.kind >= ts.SyntaxKind.FirstJSDocNode && node.kind <= ts.SyntaxKind.LastJSDocNode) return;
    const piece = PIECES.get(node.kind);
    const start = node.kind === ts.SyntaxKind.JsxText ? node.pos : node.getStart(sf);
    if (piece) found.add(`${piece} ${at[start]}..${at[node.end]}`);
    if (node.kind !== ts.SyntaxKind.JsxText) {
      ts.getLeadingCommentRanges(text, node.pos)?.forEach(comment);
      ts.getTrailingCommentRanges(text, node.end)?.forEach(comment);
    }
    node.getChildren(sf).forEach(visit);
  };
  visit(sf);
  return found;
}

function actual(path) {
  let out;
  try {
    out = execFileSync(resolve(exe), ["tokens", path], { encoding: "utf8", maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "ignore"] });
  } catch (err) {
    out = err.stdout ?? "";
  }
  const found = new Set();
  for (const line of out.split("\n")) {
    const m = /^(\d+)\.\.(\d+) (\w+) (.*)$/.exec(line);
    if (!m || !COMPARED.has(m[3])) continue;
    if (m[3] === "Comment" && m[1] === "0" && m[4].startsWith('"#!')) continue;
    if (m[3] === "JsxText" && m[1] === m[2]) continue;
    found.add(`${m[3]} ${m[1]}..${m[2]}`);
  }
  return found;
}

let files = 0, skipped = 0, bad = 0;
for (const path of walk(root)) {
  if (files >= limit) break;
  const text = readFileSync(path, "utf8");
  const want = expected(path, text);
  if (!want) { skipped++; continue; }
  files++;
  const got = actual(path);
  const missing = [...want].filter((x) => !got.has(x));
  const extra = [...got].filter((x) => !want.has(x));
  if (missing.length || extra.length) {
    bad++;
    console.log(`${path}\n  faltan: ${missing.slice(0, 5).join(", ")}\n  sobran: ${extra.slice(0, 5).join(", ")}`);
  }
}
console.log(`${files} archivos comparados, ${skipped} omitidos por errores de sintaxis, ${bad} con discrepancias`);
process.exit(bad > 0 ? 1 : 0);

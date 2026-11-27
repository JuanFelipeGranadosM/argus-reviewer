// Casos límite del lexer en TypeScript sin JSX.
type Matriz = Map<string, Array<Array<number>>>;
type Fn = <T>(x: T) => T;
const n = <number>(<unknown>"3");
const grandes = [0xFFn, 10n, 1_000_000, .5, 1e-3, 2.5E+10, 0o17, 0b1010];

let a = 10, b = 2, g = 1, i = 0;
const division = a / b / g;
const regex = /ab+c\/[/\]]+/gi;
const otra = [/x/, /y/g].map((r) => r.source);
const tras = (a) / 2 + [a][0] / 2 + i++ / 2;
const prop = { default: 4 }.default / 2;
if (/^a/.test("abc")) i += 1;

const anidada = `a${`b${`c${a}`}`}d${ { x: 1 }.x }e`;
const crudo = String.raw`C:\ruta\${a}`;
const optional = a?.toString() ?? (a ? .5 : 1);

class Contador {
  #cuenta = 0;
  static #instancias = 0;
  @registrar
  incrementar(): number {
    return ++this.#cuenta;
  }
  tiene(o: object): boolean {
    return #cuenta in o;
  }
}

function registrar(_target: unknown, _key: string) {}

const \u0061scii = "escape unicode en identificador";
const café = "identificador unicode";
let desplazado = a >> 1 >>> 0;
desplazado >>>= 1;
desplazado >>= 1;
const mayor = a >= b && a > b;
a ||= b;
a &&= b;
a ??= b;
a **= 2;

/* comentario de bloque
   en varias líneas */
export { a, b, Contador, grandes, division, regex, otra, tras, prop, anidada, crudo, optional };
export type { Matriz, Fn };

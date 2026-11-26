import os, \
    sys

numeros = [0, 0x_FF, 0o17, 0b1010, 1_000_000, 3.14, .5, 1., 1e10, 2.5E-3, 7j, 1_0.0_1e+1_0J]
cadenas = ['simple', "doble", r'\d+\'', b"bytes\x00", rb'\raw', Rb"MAYUS", u"unicode"]
triple = """linea uno
linea "dos" con 'comillas'
"""
crudo = r"""C:\ruta\sin\escape\"""
"""
anidada = f'{"x" + f"{1 + 1}"}' f"{ {'a': 1}['a'] }" f"{{literal}}" f"{3.14159:.{2}f}"
pep701 = f"{", ".join(["a", "b"])}"
nombre = f"\N{EM DASH} {os.sep!r:>10}"
plantilla = t"Hola {nombre}"
multilinea = f"""
    total: {
        sum(numeros)
    }
"""


class Base:
	def con_tab(self):
		return 1

    # comentario con indentación distinta no cambia nada


def corchetes(
    a,
        b,
  c,
):
    return {
        "a": a,

        # comentario dentro de llaves
        "b": [b,
              c],
    }


def operadores(x, y):
    x **= 2; y //= 3
    z = x @ y if x != y else ~x << 2 >> 1
    w = (lambda q: q) | x ^ y & z
    return x <= y >= z == w, ...


async def corrutina() -> dict[str, int | None]:
    async with abrir() as f:
        await f.leer()
    return {}

café = "identificador unicode"
λ = lambda: café

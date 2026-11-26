#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# Copyright 2026 Ejemplo S.A.
# SPDX-License-Identifier: MIT
"""Servicio de requisiciones generado por un agente, con los vicios típicos."""

from __future__ import annotations

import asyncio
import json  # Importar json
from dataclasses import dataclass, field
from typing import Any

#: Tasa de IVA vigente
IVA = 0.16


@dataclass
class Requisicion:
    # Definir los campos
    folio: str
    items: list[dict] = field(default_factory=list)
    urgente: bool = False


def calcular_subtotal_item(item):
    # Obtener la cantidad
    cantidad = item["cantidad"]
    # Obtener el precio
    precio = item.get("precio", 0)
    # Retornar el resultado
    return cantidad * precio


def calcular_subtotal_urgente(linea):
    # Calcular el subtotal
    qty = linea["cantidad"]
    p = linea.get("precio", 0)
    return qty * p


async def guardar(req: Requisicion, db: Any) -> None:
    try:
        db.insert(req)  # type: ignore[attr-defined]
    except Exception:
        pass
    asyncio.sleep(0.1)


def buscar(db, folio):
    # Construir la consulta
    sql = "SELECT * FROM requisiciones WHERE folio = '" + folio + "'"
    return db.execute(sql)


def resumen(req: Requisicion) -> str:
    total = sum(calcular_subtotal_item(i) for i in req.items)
    etiqueta = f"{req.folio!r}: {total * (1 + IVA):,.2f} {'URGENTE' if req.urgente else ''}"
    detalle = f"items={len(req.items)} claves={[k for k in req.items[0]] if req.items else []}"
    return f"{etiqueta} | {detalle} | {json.dumps({"folio": req.folio})}"


def clasificar(req):
    match req:
        case Requisicion(urgente=True) if (n := len(req.items)) > 10:
            return "critica", n
        case Requisicion(urgente=True):
            return "urgente", 0
        case _:
            return "normal", 0


# TODO: paginar resultados
def listar(db, pagina=1, tam=50, orden="folio", filtro=None, incluir_borrados=False):
    resultados = db.query(
        "requisiciones",
        pagina=pagina,  # noqa: E501
        tam=tam,
    )
    return [r for r in resultados if incluir_borrados or not r.borrado]


if __name__ == "__main__":
    print(resumen(Requisicion(folio="RQ-001", items=[{"cantidad": 2, "precio": 10.5}])))

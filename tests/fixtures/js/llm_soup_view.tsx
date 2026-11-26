/*!
 * Vista de requisiciones generada por un agente, con los vicios típicos.
 * SPDX-License-Identifier: MIT
 */
import { useEffect, useMemo, useState } from "react";
import type { Requisicion } from "../lib/tipos";

// Definir las props
interface Props {
  folio: string;
  items?: Array<Record<string, number>>;
  onGuardar: (req: Requisicion) => Promise<void>;
}

/** Calcula el subtotal de una línea. */
export function calcularSubtotal(item: Record<string, number>): number {
  // Retornar el resultado
  return (item.cantidad ?? 0) * (item.precio ?? 0);
}

const identidad = <T,>(valor: T): T => valor;
const primero = <T extends unknown>(lista: T[]): T | undefined => lista[0];

export default function VistaRequisicion({ folio, items = [], onGuardar }: Props) {
  // Crear el estado
  const [filtro, setFiltro] = useState<string>("");
  const [cargando, setCargando] = useState(false);

  // Calcular el total
  const total = useMemo(() => items.reduce((acc, i) => acc + calcularSubtotal(i), 0), [items]);
  const patron = /^RQ-\d{3,}$/i;
  const promedio = items.length > 0 ? total / items.length : 0;

  useEffect(() => {
    // eslint-disable-next-line react-hooks/exhaustive-deps
    setCargando(true);
  }, []);

  // @ts-ignore
  const etiqueta = `Folio ${folio} (${items.length} ${items.length === 1 ? "ítem" : `ítems`})`;

  return (
    <section className="vista" aria-label='Requisición'>
      {/* Mostrar el encabezado */}
      <h1 data-testid="titulo">{etiqueta}</h1>
      <p>
        Don't panic: el total es {total.toFixed(2)} y el promedio {promedio / 1}.
      </p>
      {cargando && <span className="spinner" />}
      <>
        <input value={filtro} onChange={(e) => setFiltro(e.target.value)} disabled={!patron.test(folio)} />
        {items
          .filter((i) => String(i.cantidad).includes(filtro))
          .map((i, n) => (
            <Fila key={n} item={i} destacado={n % 2 === 0}>
              {"Línea " + (n + 1)} &mdash; {primero([i])?.precio ?? identidad(0)}
            </Fila>
          ))}
      </>
      <button onClick={async () => { await onGuardar({ folio } as Requisicion); }}>Guardar</button>
    </section>
  );
}

function Fila(props: { item: Record<string, number>; destacado: boolean; children?: React.ReactNode }) {
  return <div className={props.destacado ? "par" : "impar"}>{props.children}</div>;
}

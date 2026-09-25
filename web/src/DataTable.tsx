import { useMemo, useState, type ReactNode } from "react";
import { IconChevron, IconSortAsc, IconSortDesc, IconSortNone } from "./icons";
import { EmptyState } from "./ui";

export interface Column<T> {
  key: string;
  header: string;
  render: (row: T) => ReactNode;
  /** Se presente la colonna è ordinabile e questo è il valore su cui si ordina. */
  sort?: (row: T) => string | number;
  align?: "left" | "right";
  width?: string;
}

interface Props<T> {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  empty: ReactNode;
  /** Testo su cui filtra la casella di ricerca; senza, la ricerca non compare. */
  searchText?: (row: T) => string;
  searchPlaceholder?: string;
  /** Contenuto dell'ultima colonna (pulsanti di riga). */
  actions?: (row: T) => ReactNode;
  /** Dettaglio a scomparsa sotto la riga; abilita la colonna con il chevron. */
  expand?: (row: T) => ReactNode;
  defaultExpanded?: (row: T) => boolean;
  initialSort?: { key: string; dir: "asc" | "desc" };
}

const cmp = (a: string | number, b: string | number) =>
  typeof a === "number" && typeof b === "number" ? a - b : String(a).localeCompare(String(b), "it", { numeric: true });

/** Tabella dati: colonne definite, ordinamento, ricerca, azioni e righe espandibili. */
export function DataTable<T>(props: Props<T>) {
  const { columns, rows, rowKey, expand } = props;
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState(props.initialSort ?? null);
  const [overrides, setOverrides] = useState<Record<string, boolean>>({});

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    let out = q && props.searchText ? rows.filter((r) => props.searchText!(r).toLowerCase().includes(q)) : rows;
    const col = sort && columns.find((c) => c.key === sort.key);
    if (col?.sort && sort) {
      const get = col.sort;
      out = [...out].sort((a, b) => (sort.dir === "asc" ? 1 : -1) * cmp(get(a), get(b)));
    }
    return out;
  }, [rows, query, sort, columns, props.searchText]);

  const toggleSort = (key: string) =>
    setSort((s) => (s?.key === key ? (s.dir === "asc" ? { key, dir: "desc" } : null) : { key, dir: "asc" }));

  const isOpen = (r: T) => overrides[rowKey(r)] ?? props.defaultExpanded?.(r) ?? false;
  const span = columns.length + (expand ? 1 : 0) + (props.actions ? 1 : 0);

  return (
    <div className="dt">
      {props.searchText && rows.length > 0 && (
        <div className="dt-tools">
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={props.searchPlaceholder ?? "Cerca…"}
            aria-label="Cerca"
          />
          {query && (
            <span className="muted small-text">
              {visible.length} di {rows.length}
            </span>
          )}
        </div>
      )}
      <div className="dt-wrap">
        <table>
          <thead>
            <tr>
              {expand && <th className="dt-chev" aria-label="Dettagli" />}
              {columns.map((c) => {
                const active = sort?.key === c.key ? sort.dir : null;
                return (
                  <th
                    key={c.key}
                    style={{ width: c.width, textAlign: c.align }}
                    aria-sort={active ? (active === "asc" ? "ascending" : "descending") : undefined}
                  >
                    {c.sort ? (
                      <button className="dt-sort" onClick={() => toggleSort(c.key)}>
                        {c.header}
                        <span className="dt-arrow" aria-hidden>
                          {active === "asc" ? <IconSortAsc /> : active === "desc" ? <IconSortDesc /> : <IconSortNone />}
                        </span>
                      </button>
                    ) : (
                      c.header
                    )}
                  </th>
                );
              })}
              {props.actions && <th className="dt-actions">Azioni</th>}
            </tr>
          </thead>
          <tbody>
            {visible.length === 0 && (
              <tr className="dt-emptyrow">
                <td colSpan={span} className="dt-none">
                  {rows.length === 0 ? (
                    props.empty
                  ) : (
                    <EmptyState image="sleeping" title="Nessun risultato" text={`Niente corrisponde a “${query}”.`} />
                  )}
                </td>
              </tr>
            )}
            {visible.map((r) => {
              const open = expand ? isOpen(r) : false;
              return (
                <RowGroup key={rowKey(r)}>
                  <tr className={open ? "open" : undefined}>
                    {expand && (
                      <td className="dt-chev">
                        <button
                          className="dt-toggle"
                          aria-expanded={open}
                          aria-label={open ? "Nascondi dettagli" : "Mostra dettagli"}
                          onClick={() => setOverrides((o) => ({ ...o, [rowKey(r)]: !open }))}
                        >
                          <IconChevron />
                        </button>
                      </td>
                    )}
                    {columns.map((c) => (
                      <td key={c.key} style={{ textAlign: c.align }}>
                        {c.render(r)}
                      </td>
                    ))}
                    {props.actions && (
                      <td className="dt-actions">
                        <div className="actions">{props.actions(r)}</div>
                      </td>
                    )}
                  </tr>
                  {open && expand && (
                    <tr className="dt-exp">
                      <td colSpan={span}>{expand(r)}</td>
                    </tr>
                  )}
                </RowGroup>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function RowGroup(props: { children: ReactNode }) {
  return <>{props.children}</>;
}

import type { ReactNode } from "react";

import { cx } from "@/components/ui";

export interface Column<T> {
  header: string;
  cell: (row: T) => ReactNode;
  align?: "right";
  /** Columns that can be dropped on narrow screens without losing the row's meaning. */
  hideBelow?: "sm" | "md" | "lg";
}

const hide = { sm: "hidden sm:table-cell", md: "hidden md:table-cell", lg: "hidden lg:table-cell" };

export function Table<T extends { id: string }>({
  rows,
  columns,
  caption,
}: {
  rows: T[];
  columns: Column<T>[];
  caption: string;
}) {
  return (
    <div className="overflow-x-auto">
      <table className="text-body w-full border-collapse text-left">
        <caption className="sr-only">{caption}</caption>
        <thead>
          <tr className="border-line border-b">
            {columns.map((col) => (
              <th
                key={col.header}
                scope="col"
                className={cx(
                  "text-meta text-muted px-4 py-2.5 font-medium whitespace-nowrap",
                  col.align === "right" && "text-right",
                  col.hideBelow && hide[col.hideBelow],
                )}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-line divide-y">
          {rows.map((row) => (
            <tr key={row.id} className="hover:bg-hover">
              {columns.map((col) => (
                <td
                  key={col.header}
                  className={cx(
                    "px-4 py-3 align-top",
                    col.align === "right" && "num text-right whitespace-nowrap",
                    col.hideBelow && hide[col.hideBelow],
                  )}
                >
                  {col.cell(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

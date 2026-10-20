import type { ReactNode } from "react";

import { cx } from "@/components/ui";

export interface Column<T> {
  header: string;
  cell: (row: T) => ReactNode;
  align?: "right";
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
    <div className="overflow-x-auto px-2">
      <table className="text-body w-full border-separate border-spacing-y-1.5 text-left">
        <caption className="sr-only">{caption}</caption>
        <thead>
          <tr>
            {columns.map((col) => (
              <th
                key={col.header}
                scope="col"
                className={cx(
                  "text-meta text-muted px-4 pt-1 pb-0.5 font-medium whitespace-nowrap",
                  col.align === "right" && "text-right",
                  col.hideBelow && hide[col.hideBelow],
                )}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.id} className="group">
              {columns.map((col) => (
                <td
                  key={col.header}
                  className={cx(
                    "bg-row group-hover:bg-hover px-4 py-3 align-middle transition-colors first:rounded-l-lg last:rounded-r-lg",
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

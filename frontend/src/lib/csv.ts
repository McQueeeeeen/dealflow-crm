export function parseCsv(text: string): string[][] {
  const source = text.replace(/^﻿/, "");
  const header = source.slice(0, source.search(/\r?\n|$/));
  const delimiter = [",", ";", "\t"].reduce((best, d) =>
    header.split(d).length > header.split(best).length ? d : best,
  );
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  const endRow = () => {
    row.push(cell);
    if (row.some((c) => c.trim())) rows.push(row);
    row = [];
    cell = "";
  };
  for (let i = 0; i < source.length; i++) {
    const ch = source[i];
    if (quoted) {
      if (ch === '"' && source[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (ch === '"') {
        quoted = false;
      } else {
        cell += ch;
      }
    } else if (ch === '"') {
      quoted = true;
    } else if (ch === delimiter) {
      row.push(cell);
      cell = "";
    } else if (ch === "\n" || ch === "\r") {
      if (ch === "\r" && source[i + 1] === "\n") i++;
      endRow();
    } else {
      cell += ch;
    }
  }
  endRow();
  return rows;
}

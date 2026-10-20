"use client";

import { IconDownload, IconUpload } from "@tabler/icons-react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { Button, Dialog, Field, FormError, Select, buttonClass, cx } from "@/components/ui";
import { post } from "@/lib/api";
import { parseCsv } from "@/lib/csv";
import type { ImportReport } from "@/lib/types";

type Kind = "companies" | "contacts";

interface Target {
  key: string;
  label: string;
  aliases: string[];
  required?: boolean;
}

const TARGETS: Record<Kind, Target[]> = {
  contacts: [
    {
      key: "first_name",
      label: "First name",
      aliases: ["firstname", "first", "givenname"],
      required: true,
    },
    { key: "last_name", label: "Last name", aliases: ["lastname", "last", "surname"] },
    { key: "email", label: "Email", aliases: ["email", "emailaddress", "mail"] },
    { key: "phone", label: "Phone", aliases: ["phone", "phonenumber", "mobile", "tel"] },
    { key: "job_title", label: "Job title", aliases: ["jobtitle", "title", "position", "role"] },
    { key: "company", label: "Company", aliases: ["company", "companyname", "organization"] },
  ],
  companies: [
    { key: "name", label: "Name", aliases: ["name", "companyname", "company"], required: true },
    { key: "domain", label: "Domain", aliases: ["domain", "website", "url", "site"] },
    { key: "industry", label: "Industry", aliases: ["industry", "sector", "vertical"] },
  ],
};

const MAX_ROWS = 1000;
const normalize = (header: string) => header.toLowerCase().replace(/[^a-z]/g, "");

export function ExportLink({ kind }: { kind: Kind | "deals" }) {
  return (
    <a href={`/api/v1/export/${kind}`} download className={buttonClass()}>
      <IconDownload size={16} stroke={1.75} aria-hidden="true" />
      Export CSV
    </a>
  );
}

export function ImportButton({ kind }: { kind: Kind }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>
        <IconUpload size={16} stroke={1.75} aria-hidden="true" />
        Import CSV
      </Button>
      <Dialog open={open} onClose={() => setOpen(false)} title={`Import ${kind}`} wide>
        {open ? <ImportFlow kind={kind} onClose={() => setOpen(false)} /> : null}
      </Dialog>
    </>
  );
}

function ImportFlow({ kind, onClose }: { kind: Kind; onClose: () => void }) {
  const client = useQueryClient();
  const targets = TARGETS[kind];
  const [file, setFile] = useState<{ name: string; header: string[]; rows: string[][] } | null>(
    null,
  );
  const [fileError, setFileError] = useState<string>();
  const [mapping, setMapping] = useState<Record<string, number>>({});

  const run = useMutation({
    mutationFn: (dryRun: boolean) =>
      post<ImportReport>(`/import/${kind}`, {
        dry_run: dryRun,
        rows: (file?.rows ?? []).map((row) =>
          Object.fromEntries(
            targets.map((t) => [t.key, row[mapping[t.key] ?? -1]?.trim() || undefined]),
          ),
        ),
      }),
    onSuccess: (report) => {
      if (!report.dry_run) {
        for (const key of [["contacts"], ["companies"], ["audit"]]) {
          client.invalidateQueries({ queryKey: key });
        }
      }
    },
  });
  const report = run.data;
  const done = report && !report.dry_run;

  async function choose(selected: File | undefined) {
    run.reset();
    setFile(null);
    setFileError(undefined);
    if (!selected) return;
    const [header = [], ...rows] = parseCsv(await selected.text());
    if (rows.length === 0) return setFileError("The file has no rows below the header.");
    if (rows.length > MAX_ROWS)
      return setFileError(`Split the file: one import takes up to ${MAX_ROWS} rows.`);
    const names = header.map(normalize);
    setMapping(
      Object.fromEntries(
        targets.map((t) => [t.key, names.findIndex((n) => t.aliases.includes(n))]),
      ),
    );
    setFile({ name: selected.name, header, rows });
  }

  const missing = targets.filter((t) => t.required && (mapping[t.key] ?? -1) < 0);

  return (
    <div className="flex flex-col gap-5">
      <Field
        label="CSV file"
        name="file"
        error={fileError}
        hint={`The first row holds column names. Up to ${MAX_ROWS} rows; matching ${kind === "contacts" ? "emails" : "domains or names"} update existing records.`}
      >
        {(p) => (
          <input
            {...p}
            type="file"
            accept=".csv,text/csv"
            onChange={(e) => choose(e.target.files?.[0])}
            className="text-body file:rounded-control file:border-line file:bg-row file:text-ink file:mr-3 file:border file:px-3 file:py-1.5"
          />
        )}
      </Field>

      {file ? (
        <section aria-label="Columns" className="flex flex-col gap-3">
          <p className="text-body text-muted">
            <span className="num text-ink font-medium">{file.rows.length}</span> rows in {file.name}
            . Match each field to a column.
          </p>
          <div className="grid gap-3 sm:grid-cols-2">
            {targets.map((t) => (
              <Field
                key={t.key}
                label={t.required ? `${t.label} (required)` : t.label}
                name={`map-${t.key}`}
              >
                {(p) => (
                  <Select
                    {...p}
                    value={mapping[t.key] ?? -1}
                    onChange={(e) => {
                      run.reset();
                      setMapping((m) => ({ ...m, [t.key]: Number(e.target.value) }));
                    }}
                  >
                    <option value={-1}>Skip</option>
                    {file.header.map((h, i) => (
                      <option key={i} value={i}>
                        {h || `Column ${i + 1}`}
                        {file.rows[0]?.[i] ? `, e.g. ${file.rows[0][i]!.slice(0, 30)}` : ""}
                      </option>
                    ))}
                  </Select>
                )}
              </Field>
            ))}
          </div>
        </section>
      ) : null}

      {report ? <Summary report={report} /> : null}
      <FormError error={run.error} />

      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="quiet" onClick={onClose}>
          {done ? "Done" : "Cancel"}
        </Button>
        {file && !done ? (
          report ? (
            <Button
              variant="primary"
              busy={run.isPending}
              disabled={report.created + report.updated === 0}
              onClick={() => run.mutate(false)}
            >
              Import {report.created + report.updated} Rows
            </Button>
          ) : (
            <Button
              variant="primary"
              busy={run.isPending}
              disabled={missing.length > 0}
              onClick={() => run.mutate(true)}
            >
              {missing.length > 0 ? `Choose a ${missing[0]!.label} Column` : "Check Rows"}
            </Button>
          )
        ) : null}
      </div>
    </div>
  );
}

function Summary({ report }: { report: ImportReport }) {
  const figures = [
    { label: report.dry_run ? "New" : "Created", value: report.created },
    { label: report.dry_run ? "Updates" : "Updated", value: report.updated },
    { label: "Unchanged", value: report.unchanged },
    { label: "Problems", value: report.errors.length },
  ];
  return (
    <section aria-live="polite" className="flex flex-col gap-3">
      <p className="font-medium">
        {report.dry_run ? "Nothing is saved yet. Here is what the import will do." : "Import done."}
      </p>
      <dl className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {figures.map((f) => (
          <div key={f.label} className="bg-row rounded-lg px-3 py-2">
            <dt className="text-meta text-muted">{f.label}</dt>
            <dd
              className={cx(
                "num text-title font-semibold",
                f.label === "Problems" && f.value > 0 && "text-critical",
              )}
            >
              {f.value}
            </dd>
          </div>
        ))}
      </dl>
      {report.companies_created > 0 ? (
        <p className="text-meta text-muted">
          {report.companies_created} new{" "}
          {report.companies_created === 1 ? "company comes" : "companies come"} from the Company
          column.
        </p>
      ) : null}
      {report.errors.length > 0 ? (
        <ul className="text-meta flex flex-col gap-1">
          {report.errors.slice(0, 8).map((e) => (
            <li key={e.row}>
              <span className="num font-medium">Line {e.row + 1}:</span>{" "}
              <span className="text-muted">{e.message}</span>
            </li>
          ))}
          {report.errors.length > 8 ? (
            <li className="text-muted">
              And {report.errors.length - 8} more. These rows are skipped.
            </li>
          ) : (
            <li className="text-muted">These rows are skipped.</li>
          )}
        </ul>
      ) : null}
    </section>
  );
}

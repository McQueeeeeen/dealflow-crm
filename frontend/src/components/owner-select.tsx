"use client";

import { Select } from "@/components/ui";
import { useMe, useUsers } from "@/lib/queries";

export function OwnerFilter({
  value,
  onChange,
  label = "Owner",
}: {
  value: string;
  onChange: (value: string) => void;
  label?: string;
}) {
  const me = useMe().data;
  const users = useUsers().data ?? [];
  return (
    <label className="text-body flex items-center gap-2">
      <span className="text-muted">{label}</span>
      <Select value={value} onChange={(e) => onChange(e.target.value)} className="w-auto">
        <option value="">Everyone</option>
        {me ? <option value={me.id}>Me</option> : null}
        {users
          .filter((u) => u.is_active && u.id !== me?.id)
          .map((u) => (
            <option key={u.id} value={u.id}>
              {u.full_name}
            </option>
          ))}
      </Select>
    </label>
  );
}

export function OwnerOptions() {
  const me = useMe().data;
  const users = useUsers().data ?? [];
  const choices = me?.role === "sales_rep" ? users.filter((u) => u.id === me.id) : users;
  return (
    <>
      {choices
        .filter((u) => u.is_active)
        .map((u) => (
          <option key={u.id} value={u.id}>
            {u.id === me?.id ? `${u.full_name} (you)` : u.full_name}
          </option>
        ))}
    </>
  );
}

"use client";

import { IconMoon, IconSun } from "@tabler/icons-react";
import { useSyncExternalStore } from "react";

import { cx } from "@/components/ui";

type Theme = "light" | "dark";

const STORAGE_KEY = "dealflow-theme";
const CHANGE_EVENT = "dealflow-theme-change";

export const themeBootScript = `try{var t=localStorage.getItem("${STORAGE_KEY}");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t}catch(e){}`;

function currentTheme(): Theme {
  return document.documentElement.dataset.theme === "light" ? "light" : "dark";
}

function subscribe(onChange: () => void) {
  window.addEventListener(CHANGE_EVENT, onChange);
  return () => window.removeEventListener(CHANGE_EVENT, onChange);
}

export function ThemeToggle({ compact }: { compact?: boolean }) {
  const theme = useSyncExternalStore(subscribe, currentTheme, () => null);
  const ThemeIcon = theme === "light" ? IconMoon : IconSun;

  function toggle() {
    const next: Theme = currentTheme() === "dark" ? "light" : "dark";
    document.documentElement.dataset.theme = next;
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {}
    window.dispatchEvent(new Event(CHANGE_EVENT));
  }

  return (
    <button
      type="button"
      onClick={toggle}
      className="text-body text-muted hover:bg-hover hover:text-ink flex min-h-10 items-center gap-3 rounded-lg px-3 font-medium"
      aria-label={theme === "light" ? "Switch to dark theme" : "Switch to light theme"}
    >
      <ThemeIcon size={18} stroke={1.75} aria-hidden="true" />
      <span className={cx(compact && "sr-only")}>
        {theme === "light" ? "Dark Theme" : "Light Theme"}
      </span>
    </button>
  );
}

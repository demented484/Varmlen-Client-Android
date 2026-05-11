import { browser } from "$app/environment";

export type Theme = "dark" | "light";
const KEY = "aegisvpn.theme";

function read(): Theme {
  if (!browser) return "dark";
  const stored = localStorage.getItem(KEY);
  return stored === "light" ? "light" : "dark";
}

function apply(t: Theme): void {
  if (!browser) return;
  document.documentElement.setAttribute("data-theme", t);
  document.documentElement.style.colorScheme = t;
}

class ThemeStore {
  current = $state<Theme>(read());

  init(): void {
    apply(this.current);
  }

  set(t: Theme): void {
    this.current = t;
    if (browser) localStorage.setItem(KEY, t);
    apply(t);
  }

  toggle(): void {
    this.set(this.current === "dark" ? "light" : "dark");
  }
}

export const theme = new ThemeStore();

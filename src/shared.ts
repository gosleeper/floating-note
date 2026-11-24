import { invoke } from "@tauri-apps/api/core";

export interface Appearance {
  font_size: number;
  width: number;
  color: string;
  opacity: number;
  outline: boolean;
}

export interface NoteState {
  text: string;
  config: Appearance;
  shortcut: string;
  defaultShortcut: string;
  shortcutOk: boolean;
  dataDir: string;
  warning: string | null;
  historyWarning: string | null;
}

export const loadState = () => invoke<NoteState>("load_state");
export const isMac = /mac/i.test(navigator.platform);

export function element<T extends HTMLElement>(id: string): T {
  const node = document.getElementById(id);
  if (!node) throw new Error(`找不到界面元素 ${id}`);
  return node as T;
}

export function paintText(node: HTMLElement, config: Appearance) {
  const [r, g, b] = config.color.slice(1).match(/../g)!.map(s => parseInt(s, 16));
  const outline = 0.299 * r + 0.587 * g + 0.114 * b > 140 ? "#000" : "#fff";
  node.style.color = config.color;
  node.style.fontSize = `${config.font_size}pt`;
  node.style.opacity = String(config.opacity);
  node.style.webkitTextStroke = config.outline ? `${Math.max(2, config.font_size * 0.12)}px ${outline}` : "";
}

export function shortcutLabel(shortcut: string) {
  return shortcut.split("+").map(part => {
    const key = part.trim();
    if (/^(super|meta|win|cmd|command)$/i.test(key)) return isMac ? "Cmd" : "Win";
    if (/^(alt|option)$/i.test(key)) return isMac ? "Option" : "Alt";
    if (/^(control|ctrl)$/i.test(key)) return "Ctrl";
    if (/^shift$/i.test(key)) return "Shift";
    if (/^Key[A-Z]$/i.test(key)) return key.slice(3).toUpperCase();
    return key.replace(/^Digit(\d)$/i, "$1").replace(/^Arrow/, "");
  }).join("+");
}

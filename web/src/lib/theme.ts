import type { ITheme } from "@xterm/xterm";

/** Tokyo Night inspired terminal palette. */
export const TERM_THEME: ITheme = {
  background: "#1a1b26",
  foreground: "#a9b1d6",
  cursor: "#c0caf5",
  cursorAccent: "#1a1b26",
  selectionBackground: "#33467c",
  black: "#32344a",
  red: "#f7768e",
  green: "#9ece6a",
  yellow: "#e0af68",
  blue: "#7aa2f7",
  magenta: "#ad8ee6",
  cyan: "#449dab",
  white: "#9699a8",
  brightBlack: "#444b6a",
  brightRed: "#ff7a93",
  brightGreen: "#b9f27c",
  brightYellow: "#ff9e64",
  brightBlue: "#7da6ff",
  brightMagenta: "#bb9af7",
  brightCyan: "#0db9d7",
  brightWhite: "#acb0d0",
};

export const TERM_FONT_FAMILY =
  '"JetBrains Mono", "Fira Code", ui-monospace, Menlo, Consolas, monospace, "Noto Color Emoji", "Apple Color Emoji", "Segoe UI Emoji"';
export const TERM_FONT_SIZE = 14;

/** Presentation helpers shared by the dashboard and history views. */

const RELATIVE = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
const TIME = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
});
const DATE = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
});

/** "just now", "4 min ago", "14:32", "Mar 3" — whichever reads best. */
export function relativeTime(epochMs: number): string {
  const seconds = Math.round((epochMs - Date.now()) / 1000);
  const absolute = Math.abs(seconds);

  if (absolute < 45) return "just now";
  if (absolute < 3600) return RELATIVE.format(Math.round(seconds / 60), "minute");

  const date = new Date(epochMs);
  const isToday = new Date().toDateString() === date.toDateString();
  return isToday ? TIME.format(date) : DATE.format(date);
}

/** Trim a transcript to a preview without cutting mid-word. */
export function preview(text: string, maxLength = 120): string {
  const collapsed = text.replace(/\s+/g, " ").trim();
  if (collapsed.length <= maxLength) return collapsed;
  const cut = collapsed.slice(0, maxLength);
  const lastSpace = cut.lastIndexOf(" ");
  return `${cut.slice(0, lastSpace > 40 ? lastSpace : maxLength)}…`;
}

/**
 * Render a Tauri accelerator as macOS glyphs: "Alt+Space" -> ["⌥", "Space"].
 */
export function shortcutKeys(accelerator: string): string[] {
  const glyphs: Record<string, string> = {
    cmd: "⌘",
    command: "⌘",
    super: "⌘",
    cmdorctrl: "⌘",
    commandorcontrol: "⌘",
    ctrl: "⌃",
    control: "⌃",
    alt: "⌥",
    option: "⌥",
    shift: "⇧",
  };

  // Punctuation keys read as the character, not their accelerator name.
  const literals: Record<string, string> = {
    period: ".",
    comma: ",",
    slash: "/",
    semicolon: ";",
    quote: "'",
    backquote: "`",
    minus: "-",
    equal: "=",
    bracketleft: "[",
    bracketright: "]",
    backslash: "\\",
  };

  return accelerator
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const key = part.toLowerCase();
      return glyphs[key] ?? literals[key] ?? part;
    });
}

/** 24-hour clock, matching the session log on the marketing site. */
export function clockTime(epochMs: number): string {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date(epochMs));
}

export function wordCount(text: string): number {
  const trimmed = text.trim();
  return trimmed ? trimmed.split(/\s+/).length : 0;
}

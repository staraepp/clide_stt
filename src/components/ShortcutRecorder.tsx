import { useEffect, useState } from "react";
import { Keys } from "./Keys";
import { Button } from "./Button";
import { cn } from "@/lib/cn";

/**
 * Captures a key combination and returns it as a Tauri accelerator.
 *
 * Recording happens in the webview, but the accelerator is only ever *applied*
 * by Rust: `set_shortcut` tries to register it with macOS and reports back if
 * another application already owns it.
 */

const MODIFIERS = new Set(["Meta", "Control", "Alt", "Shift"]);

/** DOM `event.code` -> the token Tauri's parser expects. */
function codeToKey(code: string): string | null {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;

  const named: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Escape: "Escape",
    Tab: "Tab",
    Backquote: "Backquote",
    Minus: "Minus",
    Equal: "Equal",
    BracketLeft: "BracketLeft",
    BracketRight: "BracketRight",
    Backslash: "Backslash",
    Semicolon: "Semicolon",
    Quote: "Quote",
    Comma: "Comma",
    Period: "Period",
    Slash: "Slash",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
  };
  return named[code] ?? null;
}

export function ShortcutRecorder({
  value,
  onChange,
  className,
}: {
  value: string;
  onChange: (accelerator: string) => void;
  className?: string;
}) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  useEffect(() => {
    if (!recording) return;

    const onKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();

      if (event.key === "Escape") {
        setRecording(false);
        setHint(null);
        return;
      }
      if (MODIFIERS.has(event.key)) {
        // Waiting for the non-modifier key.
        return;
      }

      const parts: string[] = [];
      if (event.metaKey) parts.push("Cmd");
      if (event.ctrlKey) parts.push("Ctrl");
      if (event.altKey) parts.push("Alt");
      if (event.shiftKey) parts.push("Shift");

      const key = codeToKey(event.code);
      if (!key) {
        setHint("That key can't be used in a shortcut.");
        return;
      }

      // A bare letter would fire every time the user types anywhere.
      if (parts.length === 0) {
        setHint("Add at least one modifier, like ⌥ or ⌃.");
        return;
      }

      parts.push(key);
      setRecording(false);
      setHint(null);
      onChange(parts.join("+"));
    };

    window.addEventListener("keydown", onKeyDown, { capture: true });
    return () =>
      window.removeEventListener("keydown", onKeyDown, { capture: true });
  }, [recording, onChange]);

  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => {
            setRecording((active) => !active);
            setHint(null);
          }}
          className={cn(
            "no-drag flex h-10 min-w-[160px] items-center justify-center gap-2 rounded-ctl border px-4 transition-colors",
            recording
              ? "border-voice bg-voice-tint"
              : "border-line-2 bg-card hover:bg-sunken",
          )}
        >
          {recording ? (
            <span className="text-[13px] text-voice-deep">
              Press a combination…
            </span>
          ) : (
            <Keys accelerator={value} />
          )}
        </button>

        {recording && (
          <Button size="sm" variant="ghost" onClick={() => setRecording(false)}>
            Cancel
          </Button>
        )}
      </div>

      {hint && <p className="text-[12px] text-warn">{hint}</p>}
    </div>
  );
}

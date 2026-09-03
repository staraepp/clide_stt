import { cn } from "@/lib/cn";

export type Tone = "ready" | "pending" | "busy" | "problem" | "idle";

/**
 * The one status indicator in the app, so a colour always means the same thing.
 *
 * `busy` is the only tone that uses the voice blue, and it is only ever set
 * while clide is actually handling speech.
 */
const TONE_CLASS: Record<Tone, string> = {
  ready: "bg-ok",
  busy: "bg-voice",
  pending: "bg-warn",
  problem: "bg-stop",
  idle: "bg-line-2",
};

export function StatusDot({
  tone,
  pulse,
  className,
}: {
  tone: Tone;
  pulse?: boolean;
  className?: string;
}) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-block size-1.5 shrink-0 rounded-full",
        TONE_CLASS[tone],
        pulse && "animate-pulse",
        className,
      )}
    />
  );
}

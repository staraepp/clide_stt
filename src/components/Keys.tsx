import { shortcutKeys } from "@/lib/format";
import { cn } from "@/lib/cn";

/** A shortcut rendered as macOS keycaps. */
export function Keys({
  accelerator,
  className,
}: {
  accelerator: string;
  className?: string;
}) {
  const keys = shortcutKeys(accelerator);

  if (keys.length === 0) {
    return <span className="text-ink-3">Not set</span>;
  }

  return (
    <span className={cn("inline-flex items-center gap-1", className)}>
      {keys.map((key, index) => (
        <kbd key={`${key}-${index}`} className="key">
          {key}
        </kbd>
      ))}
    </span>
  );
}

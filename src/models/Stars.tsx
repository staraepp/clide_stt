import { cn } from "@/lib/cn";

/**
 * A 0–5 rating in half stars.
 *
 * Every value shown here is derived in Rust from the model's declared class and
 * this Mac's measured memory and chip — never from a popularity score, which
 * Clide has no telemetry to know.
 */
export function Stars({
  value,
  label,
  className,
}: {
  value: number;
  label: string;
  className?: string;
}) {
  return (
    <span
      className={cn("inline-flex items-center gap-1.5", className)}
      role="img"
      aria-label={`${label}: ${value} out of 5`}
    >
      <span aria-hidden className="flex items-center gap-[1.5px]">
        {[0, 1, 2, 3, 4].map((index) => {
          const filled = Math.max(0, Math.min(1, value - index));
          return (
            <span key={index} className="relative block h-[9px] w-[9px]">
              <Star className="absolute inset-0 text-line-2" fill />
              {filled > 0 && (
                <span
                  className="absolute inset-0 overflow-hidden"
                  style={{ width: `${filled * 100}%` }}
                >
                  <Star className="h-[9px] w-[9px] text-ink" fill />
                </span>
              )}
            </span>
          );
        })}
      </span>
      <span className="numeral text-[10.5px] text-ink-3">{value.toFixed(1)}</span>
    </span>
  );
}

function Star({ className, fill }: { className?: string; fill?: boolean }) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden>
      <path
        d="M12 2.5l2.9 6.1 6.6.9-4.8 4.6 1.2 6.6L12 17.6l-5.9 3.1 1.2-6.6L2.5 9.5l6.6-.9z"
        fill={fill ? "currentColor" : "none"}
      />
    </svg>
  );
}

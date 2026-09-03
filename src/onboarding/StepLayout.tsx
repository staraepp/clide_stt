import type { ReactNode } from "react";

/** Shared frame for the onboarding steps that ask the user to do something. */
export function StepLayout({
  icon,
  title,
  description,
  children,
}: {
  icon: ReactNode;
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div>
      <div className="flex items-center gap-3">
        <span className="flex size-9 items-center justify-center rounded-xl border border-line bg-sunken text-voice-deep">
          {icon}
        </span>
        <h1 className="display text-[19px] text-ink">
          {title}
        </h1>
      </div>

      <p className="mt-3 text-[13px] leading-relaxed text-ink-2">
        {description}
      </p>

      <div className="mt-5">{children}</div>
    </div>
  );
}

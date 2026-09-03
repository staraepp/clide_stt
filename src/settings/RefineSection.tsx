import { useEffect, useState } from "react";
import { Sparkles } from "lucide-react";

import { Segmented } from "@/components/Segmented";
import { StatusDot } from "@/components/StatusDot";
import * as commands from "@/lib/commands";
import type { RefinerDescriptor, SystemStatus } from "@/lib/types";

/**
 * Rewrite mode's engine and how far it may go.
 *
 * Lists every engine whether or not it is usable: an option that silently
 * disappears when Apple Intelligence is switched off leaves the user with no
 * way to understand why Rewrite stopped working.
 */
export function RefineSection({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const [refiners, setRefiners] = useState<RefinerDescriptor[]>([]);

  useEffect(() => {
    commands
      .listRefiners()
      .then(setRefiners)
      .catch((error) => console.error("could not list refiners", error));
  }, [status.settings.mode]);

  return (
    <div className="flex flex-col gap-4">
      <Segmented
        className="w-full"
        value={status.settings.refineStyle}
        onChange={async (style) => {
          await commands.setRefineStyle(style);
          refresh();
        }}
        segments={[
          {
            value: "tidy",
            label: "Tidy it up",
            hint: "Punctuation and slips only — your wording is kept",
          },
          {
            value: "written",
            label: "Make it written",
            hint: "Spoken phrasing becomes written prose",
          },
        ]}
      />

      <ul className="grid gap-2 sm:grid-cols-2">
        {refiners.map((refiner) => (
          <li
            key={refiner.id}
            className="flex flex-col gap-2 rounded-ctl border border-line bg-card px-3 py-3"
          >
            <span className="flex items-center gap-2">
              <Sparkles size={13} className="shrink-0 text-ink-3" />
              <span className="truncate text-[13px] text-ink">{refiner.name}</span>
              <span className="ml-auto flex shrink-0 items-center gap-1.5">
                <StatusDot tone={refiner.available ? "ready" : "pending"} />
                <span className="text-[11px] text-ink-3">
                  {refiner.available ? "Ready" : "Off"}
                </span>
              </span>
            </span>
            <p className="text-[11.5px] leading-relaxed text-ink-3">
              {refiner.available ? refiner.description : refiner.unavailableReason}
            </p>
          </li>
        ))}
      </ul>

      <p className="text-[12px] leading-relaxed text-ink-3">
        If no engine is available, Rewrite still returns your polished
        transcript — refinement never costs you your words.
      </p>
    </div>
  );
}

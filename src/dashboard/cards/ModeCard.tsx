import { Card, CardHeader } from "@/components/Card";
import { Segmented } from "@/components/Segmented";
import * as commands from "@/lib/commands";
import type { ProcessingMode } from "@/lib/types";

const DESCRIPTIONS: Record<ProcessingMode, string> = {
  verbatim: "Your words as spoken. Spacing is tidied, nothing else.",
  polished: "Local cleanup — fillers, stutters, spacing. No model, no delay.",
  rewrite: "Rewrites speech into written prose. Coming in a later version.",
};

/**
 * Processing style.
 *
 * Rewrite is visible but disabled: it is a real part of the product and hiding
 * it would misrepresent what clide is, but it is not implemented, and the
 * backend refuses it rather than quietly falling back to Polished.
 */
export function ModeCard({
  mode,
  onChange,
}: {
  mode: ProcessingMode;
  onChange: () => void;
}) {
  return (
    <Card index={2} className="col-span-12 flex flex-col p-4.5 lg:col-span-4">
      <CardHeader label="Style" />

      <Segmented
        className="mt-3 w-full"
        value={mode}
        onChange={async (next) => {
          await commands.setProcessingMode(next);
          onChange();
        }}
        segments={[
          { value: "verbatim", label: "Verbatim" },
          { value: "polished", label: "Polished" },
          {
            value: "rewrite",
            label: "Rewrite",
            disabled: true,
            hint: "Coming in a later version",
          },
        ]}
      />

      <p className="mt-3 text-[12.5px] leading-relaxed text-ink-2">
        {DESCRIPTIONS[mode]}
      </p>
    </Card>
  );
}

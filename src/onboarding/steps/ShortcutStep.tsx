import { Command } from "lucide-react";
import { useState } from "react";
import { Segmented } from "@/components/Segmented";
import { ShortcutRecorder } from "@/components/ShortcutRecorder";
import * as commands from "@/lib/commands";
import type { SystemStatus } from "@/lib/types";
import { StepLayout } from "../StepLayout";

export function ShortcutStep({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const [error, setError] = useState<string | null>(null);

  return (
    <StepLayout
      icon={<Command size={18} />}
      title="Pick your shortcut"
      description="One shortcut does everything. Choose whether holding it records, or whether one press starts and the next one stops."
    >
      <Segmented
        className="w-full"
        value={status.settings.behavior}
        onChange={async (behavior) => {
          await commands.setDictationBehavior(behavior);
          refresh();
        }}
        segments={[
          { value: "hold", label: "Hold to talk" },
          { value: "toggle", label: "Press to toggle" },
        ]}
      />

      <div className="mt-4">
        <ShortcutRecorder
          value={status.settings.shortcut}
          onChange={async (accelerator) => {
            setError(null);
            try {
              await commands.setShortcut(accelerator);
              refresh();
            } catch (caught) {
              setError(commands.errorMessage(caught));
            }
          }}
        />
        {error && (
          <p className="mt-2 text-[12px] text-stop">{error}</p>
        )}
        {!error && !status.shortcutRegistered && (
          <p className="mt-2 text-[12px] text-warn">
            That combination isn't available. Try another.
          </p>
        )}
      </div>
    </StepLayout>
  );
}

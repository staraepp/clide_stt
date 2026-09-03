import { Keyboard } from "lucide-react";
import { Button } from "@/components/Button";
import { StatusDot } from "@/components/StatusDot";
import * as commands from "@/lib/commands";
import type { PermissionStatus } from "@/lib/types";
import { StepLayout } from "../StepLayout";

export function AccessibilityStep({
  status,
  refresh,
}: {
  status: PermissionStatus;
  refresh: () => void;
}) {
  const granted = status === "granted";

  return (
    <StepLayout
      icon={<Keyboard size={18} />}
      title="Let clide type for you"
      description="Accessibility access is how clide places text into the app you're working in. clide uses it to write where your cursor is — it doesn't read your screen."
    >
      <div className="flex items-center gap-3">
        <StatusDot tone={granted ? "ready" : "pending"} />
        <span className="text-[13px] text-ink-2">
          {granted
            ? "Accessibility access granted."
            : "Turn on clide in Privacy & Security → Accessibility."}
        </span>

        {!granted && (
          <Button
            variant="primary"
            className="ml-auto"
            onClick={async () => {
              // Shows the system prompt if macOS has never asked, and opens
              // Settings either way — the prompt only appears once.
              await commands.requestAccessibilityPermission();
              await commands.openAccessibilitySettings();
              refresh();
            }}
          >
            Open System Settings
          </Button>
        )}
      </div>

      {!granted && (
        <p className="mt-3 text-[12px] leading-relaxed text-ink-3">
          Come back here after switching it on — clide re-checks automatically.
        </p>
      )}
    </StepLayout>
  );
}

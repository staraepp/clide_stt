import { Mic } from "lucide-react";
import { Button } from "@/components/Button";
import { StatusDot } from "@/components/StatusDot";
import * as commands from "@/lib/commands";
import type { PermissionStatus } from "@/lib/types";
import { StepLayout } from "../StepLayout";

export function MicrophoneStep({
  status,
  refresh,
}: {
  status: PermissionStatus;
  refresh: () => void;
}) {
  const granted = status === "granted";
  const denied = status === "denied" || status === "restricted";

  return (
    <StepLayout
      icon={<Mic size={18} />}
      title="Let clide hear you"
      description="clide records only while you're holding the dictation shortcut. Recordings are deleted as soon as the text comes back — nothing is kept."
    >
      <div className="flex items-center gap-3">
        <StatusDot tone={granted ? "ready" : denied ? "problem" : "pending"} />
        <span className="text-[13px] text-ink-2">
          {granted
            ? "Microphone access granted."
            : denied
              ? "macOS is blocking the microphone for clide."
              : "Not granted yet."}
        </span>

        {!granted && (
          <Button
            variant="primary"
            className="ml-auto"
            onClick={async () => {
              if (denied) {
                commands.openMicrophoneSettings();
              } else {
                await commands.requestMicrophonePermission();
              }
              refresh();
            }}
          >
            {denied ? "Open System Settings" : "Allow microphone"}
          </Button>
        )}
      </div>
    </StepLayout>
  );
}

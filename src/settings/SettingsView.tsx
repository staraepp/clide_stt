import { useState } from "react";
import { Segmented } from "@/components/Segmented";
import { ShortcutRecorder } from "@/components/ShortcutRecorder";
import { Button } from "@/components/Button";
import { ProviderSettings } from "@/providers/ProviderSettings";
import * as commands from "@/lib/commands";
import type { SystemStatus } from "@/lib/types";

export function SettingsView({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const [shortcutError, setShortcutError] = useState<string | null>(null);

  return (
    <div className="scroll-area -mr-2 flex h-full flex-col gap-6 pb-10 pr-2">
      <Section
        title="Shortcut"
        description="One shortcut, used everywhere. Hold to talk, or press once to start and again to stop."
      >
        <div className="flex flex-wrap items-start gap-6">
          <div>
            <ShortcutRecorder
              value={status.settings.shortcut}
              onChange={async (accelerator) => {
                setShortcutError(null);
                try {
                  await commands.setShortcut(accelerator);
                  refresh();
                } catch (error) {
                  setShortcutError(commands.errorMessage(error));
                }
              }}
            />
            {shortcutError && (
              <p className="mt-2 max-w-[280px] text-[12px] text-stop">
                {shortcutError}
              </p>
            )}
            {!status.shortcutRegistered && !shortcutError && (
              <p className="mt-2 max-w-[280px] text-[12px] text-warn">
                This shortcut isn't active. Another app may already be using it.
              </p>
            )}
          </div>

          <Segmented
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
        </div>
      </Section>

      <Section
        title="Transcription"
        description="clide is bring-your-own-key. Keys are stored on this Mac only, in a file just your account can read. They never reach clide's database, its settings, or any log."
      >
        <ProviderSettings onChange={refresh} />
      </Section>

      <Section
        title="Visual effects"
        description="How much motion the background shader is allowed. macOS Reduce Motion always wins over this setting."
      >
        <Segmented
          value={status.settings.visualIntensity}
          onChange={async (intensity) => {
            await commands.setVisualIntensity(intensity);
            refresh();
          }}
          segments={[
            { value: "reduced", label: "Reduced", hint: "Static background" },
            { value: "normal", label: "Normal", hint: "Slow ambient drift" },
            { value: "high", label: "High", hint: "Reacts to your voice" },
          ]}
        />
      </Section>

      <Section
        title="Permissions"
        description="clide needs the microphone to hear you and Accessibility to type into other applications."
      >
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => commands.openMicrophoneSettings()}>
            Microphone settings
          </Button>
          <Button onClick={() => commands.openAccessibilitySettings()}>
            Accessibility settings
          </Button>
          <Button
            variant="ghost"
            onClick={async () => {
              await commands.resetOnboarding();
              refresh();
            }}
          >
            Run setup again
          </Button>
        </div>
      </Section>
    </div>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3">
      <div>
        <h2 className="display text-[15px] text-ink">
          {title}
        </h2>
        <p className="mt-1 max-w-[560px] text-[12.5px] leading-relaxed text-ink-2">
          {description}
        </p>
      </div>
      {children}
    </section>
  );
}

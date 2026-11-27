import { useEffect, useState } from "react";
import { EVENTS, on } from "@/lib/events";
import { Cpu } from "lucide-react";
import { Button } from "@/components/Button";
import { StatusDot } from "@/components/StatusDot";
import * as commands from "@/lib/commands";
import type { SystemStatus } from "@/lib/types";
import { StepLayout } from "../StepLayout";

const BUILT_IN_ENGINE = "apple";

export function ProviderStep({
  status,
  refresh,
}: {
  status: SystemStatus;
  refresh: () => void;
}) {
  const windows = /Win/i.test(navigator.platform);
  const [downloading, setDownloading] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!downloading || status.providerReady) return;
    const timer = setInterval(refresh, 2000);
    return () => clearInterval(timer);
  }, [downloading, status.providerReady, refresh]);
  useEffect(() => {
    const subscriptions = [
      on(EVENTS.modelFailed, (event) => { if (event.modelId === status.settings.modelId) { setDownloading(false); setError(event.message); } }),
      on(EVENTS.modelComplete, (modelId) => { if (modelId === status.settings.modelId) { setDownloading(false); refresh(); } }),
    ];
    return () => subscriptions.forEach((sub) => { void sub.then((unlisten) => unlisten()); });
  }, [status.settings.modelId, refresh]);
  const usingBuiltIn = status.settings.providerId === BUILT_IN_ENGINE;

  return (
    <StepLayout
      icon={<Cpu size={18} />}
      title="Choose how clide listens"
      description="clide runs entirely on this device. Your audio is never uploaded, and there is no account or key to set up."
    >
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-3 rounded-ctl border border-line bg-sunken/60 px-3 py-3">
          <StatusDot tone={status.providerReady ? "ready" : "pending"} />
          <div className="min-w-0">
            <p className="text-[13.5px] text-ink">{status.providerName}</p>
            <p className="text-[12px] text-ink-3">
              {status.modelName}
              {status.providerReady ? "" : " · not downloaded yet"}
            </p>
          </div>
          {!windows && !usingBuiltIn && (
            <Button
              className="ml-auto"
              size="sm"
              onClick={async () => {
                await commands.selectProvider(BUILT_IN_ENGINE);
                refresh();
              }}
            >
              Use Apple Speech
            </Button>
          )}
        </div>

        <p className="text-[12px] leading-relaxed text-ink-3">
          {windows ? "Download a local speech model to begin. Whisper Base is the initial CPU-friendly choice. You can install other Whisper or Parakeet models from Models later." : "Apple Speech is built into macOS and works right now. For higher accuracy you can download Whisper or Parakeet models from the Models page whenever you like."}
        </p>
        {windows && !status.providerReady && <Button disabled={downloading} onClick={async () => {
          setDownloading(true); setError("");
          try { await commands.downloadModel(status.settings.modelId); refresh(); }
          catch (e) { setDownloading(false); setError(String(e)); }
        }}>{downloading ? "Downloading model…" : "Download speech model"}</Button>}
        {error && <p role="alert">{error}</p>}
      </div>
    </StepLayout>
  );
}

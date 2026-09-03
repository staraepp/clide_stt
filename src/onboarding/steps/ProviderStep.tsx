import { Cloud } from "lucide-react";
import { ProviderSettings } from "@/providers/ProviderSettings";
import { StepLayout } from "../StepLayout";

export function ProviderStep({ refresh }: { refresh: () => void }) {
  return (
    <StepLayout
      icon={<Cloud size={18} />}
      title="Connect a transcription engine"
      description="clide is bring-your-own-key. Your key is verified once and then kept on this Mac, in a file only your account can read. It never touches clide's database or logs."
    >
      <ProviderSettings onChange={refresh} />
    </StepLayout>
  );
}

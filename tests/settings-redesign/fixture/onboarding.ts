import { useState } from "react";
import { BaseCodingAgent, SoundFile } from "../../../shared/types";
const params = new URLSearchParams(location.search);
const writes: unknown[] = [];
const navigations: unknown[] = [];
Object.defineProperty(window, "__onboardingWrites", { value: writes });
Object.defineProperty(window, "__onboardingNavigations", {
  value: navigations,
});
const navigation = {
  goToRoot: (options: unknown) => navigations.push(options),
};
export function useAppNavigation() {
  return navigation;
}
export function useUserSystem() {
  const [config, setConfig] = useState({
    onboarding_acknowledged: false,
    disclaimer_acknowledged: false,
    executor_profile: { executor: BaseCodingAgent.CLAUDE_CODE, variant: null },
    editor: {
      entries: [
        { name: "Raw editor 原文", uri_template: "vscode://file/{path}" },
      ],
    },
    notifications: {
      sound_enabled: true,
      sound_file: SoundFile.COW_MOOING,
      push_enabled: false,
    },
    hidden_agents: [],
    language: "BROWSER",
  });
  return {
    config,
    loading: false,
    profiles: { CLAUDE_CODE: {}, CODEX: {} },
    updateAndSaveConfig: async (
      patch: typeof config,
      onError: (value: string) => void,
    ) => {
      writes.push(patch);
      await new Promise<void>((resolve) =>
        Object.defineProperty(window, "__finishOnboarding", {
          value: resolve,
          configurable: true,
        }),
      );
      if (params.has("onboarding-error")) {
        onError("Raw provider secret / internal-path");
        return false;
      }
      setConfig((previous) => ({ ...previous, ...patch }));
      return true;
    },
  };
}

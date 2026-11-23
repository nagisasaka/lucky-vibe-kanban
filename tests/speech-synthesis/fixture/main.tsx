import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { createRoot } from "react-dom/client";
import { useState } from "react";
import en from "../../../packages/web-core/src/i18n/locales/en/settings.json";
import ja from "../../../packages/web-core/src/i18n/locales/ja/settings.json";
import common from "../../../packages/web-core/src/i18n/locales/en/common.json";
import { SpeechSynthesisSettingsSection } from "../../../packages/web-core/src/shared/dialogs/settings/settings/SpeechSynthesisSettingsSection";
import { RunSpeechToggle } from "../../../packages/web-core/src/features/kanban/ui/RunSpeechToggle";
import "../../../packages/web-core/src/app/styles/new/index.css";
i18n.use(initReactI18next).init({
  lng: new URLSearchParams(location.search).get("lang") ?? "en",
  resources: { en: { settings: en, common }, ja: { settings: ja, common } },
  defaultNS: "settings",
});
function Fixture() {
  const [settings, setSettings] = useState(true);
  return (
    <main className="p-double max-w-3xl">
      <RunSpeechToggle projectId="fixture" />
      <button onClick={() => setSettings(!settings)}>Toggle settings</button>
      {settings && <SpeechSynthesisSettingsSection />}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

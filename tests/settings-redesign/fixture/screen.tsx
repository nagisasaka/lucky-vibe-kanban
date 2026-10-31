import { useState } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HotkeysProvider } from "../../../packages/web-core/node_modules/react-hotkeys-hook";
import { Provider } from "@ebay/nice-modal-react";
import i18n from "@/i18n/config";
import { AppRuntimeProvider } from "@/shared/hooks/useAppRuntime";
import { AuthContext } from "@/shared/hooks/auth/useAuth";
import { SettingsDialog } from "@/shared/dialogs/settings/SettingsDialog";
import type { SettingsSectionType } from "@/shared/dialogs/settings/settings/settingsRegistry";
import { ServerAccessGate } from "../../../packages/local-web/src/app/auth/ServerAccessGate";
import "@/app/styles/new/index.css";

const params = new URLSearchParams(location.search);
void i18n.changeLanguage(params.get("lang") ?? "ja");
if (params.get("theme") === "dark")
  document.documentElement.classList.add("dark");
function Screen() {
  const [opened, setOpened] = useState(false);
  const open = () => {
    setOpened(true);
    void SettingsDialog.show({
      initialSection: (params.get("section") ??
        undefined) as SettingsSectionType,
    });
  };
  return (
    <div className="p-double text-normal bg-primary min-h-screen">
      <h1 className="text-lg text-high">
        設定UI試作 / Isolated settings prototype
      </h1>
      <p>
        サンプルデータのみ。保存はこのページのメモリ内です。再読込でリセットします。実認証の変更・実行・復旧は無効です。
      </p>
      <p>
        Only sample data. Saves last until reload. Authentication changes and
        recovery are disabled.
      </p>
      <button className="rounded border p-base mt-double" onClick={open}>
        {opened ? "設定を開き直す" : "設定を開く"}
      </button>
    </div>
  );
}
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    <AppRuntimeProvider runtime="local">
      <AuthContext.Provider
        value={{ isSignedIn: false, isLoaded: true, userId: null }}
      >
        <HotkeysProvider initiallyActiveScopes={["*"]}>
          <ServerAccessGate>
            <Provider>
              <Screen />
            </Provider>
          </ServerAccessGate>
        </HotkeysProvider>
      </AuthContext.Provider>
    </AppRuntimeProvider>
  </QueryClientProvider>,
);

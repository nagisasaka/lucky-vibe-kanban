// Production version controller and Lexical editor; only HTTP is mocked by the runner.
import { useState } from "react";
import { createRoot } from "react-dom/client";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import common from "../../packages/web-core/src/i18n/locales/en/common.json";
import { IssueVersionEditor } from "../../packages/web-core/src/pages/kanban/IssueVersionEditor";

i18n
  .use(initReactI18next)
  .init({ lng: "en", resources: { en: { common } }, initImmediate: false });
const client = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});
function Fixture() {
  const [revision, setRevision] = useState("1");
  const [current, setCurrent] = useState<string | null>(null);
  Object.assign(window, { observeRevision: setRevision });
  return (
    <QueryClientProvider client={client}>
      <main className="bg-primary text-normal p-base">
        <output id="current-description">{current}</output>
        <IssueVersionEditor
          issueId="issue"
          projectId="project"
          observedIssueRevision={revision}
          onCurrentDescription={setCurrent}
          editorProps={{
            value: "",
            disabled: false,
            plainText: true,
            hideActions: true,
          }}
        />
      </main>
    </QueryClientProvider>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

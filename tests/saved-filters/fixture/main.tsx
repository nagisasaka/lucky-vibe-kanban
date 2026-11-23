import { TemporaryFilters } from "./temporary-filters";
import { Integration } from "./integration";
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import "@/i18n/config";
import i18n from "i18next";
import "@/app/styles/new/index.css";
import { SavedFilterEditor } from "@/shared/components/saved-filters/SavedFilterEditor";
import { SavedFilterStatus } from "@/shared/components/saved-filters/SavedFilterStatus";
import { emptyDefinition } from "@/shared/lib/savedFilters";
i18n.changeLanguage("ja");
function App() {
  const [value, setValue] = useState({
    ...emptyDefinition(),
    name: "今回の公開を難しくしている仕事",
    natural_language: "公開をブロックする未解決の仕事",
    context: "v2の公開。認証と移行の問題を含む。",
  });
  const [state, setState] = useState("unevaluated");
  const [stale, setStale] = useState(false);
  const [dark, setDark] = useState(false);
  return (
    <main className={dark ? "dark" : ""}>
      <div className="min-h-screen bg-primary p-double text-normal">
        <div className="mx-auto max-w-3xl space-y-double">
          <button onClick={() => setDark(!dark)}>Theme</button>
          <h1 className="text-xl text-high">カンバンボードのフィルター</h1>
          <div className="space-y-base rounded-sm border border-border p-base">
            <label>
              評価状態
              <select
                aria-label="評価状態"
                value={state}
                onChange={(e) => setState(e.target.value)}
              >
                {["unevaluated", "evaluating", "complete", "failed"].map(
                  (s) => (
                    <option key={s}>{s}</option>
                  ),
                )}
              </select>
            </label>
            <label>
              <input
                type="checkbox"
                checked={stale}
                onChange={(e) => setStale(e.target.checked)}
              />
              前回結果
            </label>
            <SavedFilterStatus
              count={2}
              view={
                {
                  state,
                  stale,
                  ready: stale || state === "complete",
                  missingIssues: false,
                  evaluation: {
                    isFetching: state === "evaluating",
                    data: {
                      error:
                        state === "failed"
                          ? "評価サービスに接続できません"
                          : null,
                    },
                  },
                  definition: { isError: false },
                } as never
              }
            />
          </div>
          <form onSubmit={(e) => e.preventDefault()}>
            <SavedFilterEditor
              value={value}
              onChange={setValue}
              options={{
                statuses: [
                  { id: "todo", label: "Todo" },
                  { id: "progress", label: "In Progress" },
                  { id: "done", label: "Done" },
                ],
                assignees: [
                  { id: "alice", label: "Alice" },
                  { id: "bob", label: "Bob" },
                ],
                tags: [
                  { id: "bug", label: "bug" },
                  { id: "release", label: "release" },
                ],
                issues: [{ id: "release", label: "LOCAL-460 公開作業" }],
              }}
            />
            <button type="submit">保存</button>
          </form>
          <div hidden data-testid="definition">
            {JSON.stringify(value)}
          </div>
        </div>
      </div>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  new URLSearchParams(location.search).has("temporary-filters") ? (
    <TemporaryFilters />
  ) : new URLSearchParams(location.search).has("integration") ? (
    <Integration />
  ) : (
    <App />
  ),
);

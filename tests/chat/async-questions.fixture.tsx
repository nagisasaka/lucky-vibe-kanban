import { useState } from "react";
import { createRoot } from "react-dom/client";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { AsyncAgentQuestions } from "../../packages/web-core/src/features/agent-runtime/ui/AsyncAgentQuestions";
import common from "../../packages/web-core/src/i18n/locales/ja/common.json";

void i18n
  .use(initReactI18next)
  .init({ lng: "ja", resources: { ja: { common } }, initImmediate: false });

function Fixture() {
  const [status, setStatus] = useState<string | null>(null);
  const [expired, setExpired] = useState(false);
  const [calls, setCalls] = useState<Array<{ answer: string; retry: boolean }>>(
    [],
  );
  return (
    <main>
      <button onClick={() => setExpired(true)}>Expire</button>
      <AsyncAgentQuestions
        scope="fixture"
        disabled={false}
        questions={
          expired
            ? []
            : [
                {
                  id: "attempt:message:0",
                  title: "Colour?",
                  options: ["Blue"],
                  delivery_status: status,
                },
              ]
        }
        onAnswer={async (_question, answer, retry) => {
          setCalls((current) => [...current, { answer, retry }]);
          if (!retry) {
            setStatus("unconfirmed");
            throw new Error("unknown");
          }
        }}
      />
      <output>{JSON.stringify(calls)}</output>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

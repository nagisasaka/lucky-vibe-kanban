import { useState } from "react";
import { createRoot } from "react-dom/client";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { AskUserQuestionBanner } from "../../packages/ui/src/components/AskUserQuestionBanner";
import { nativeInputQuestions } from "../../packages/web-core/src/features/agent-runtime/model/nativeAgentQuestions";
import common from "../../packages/web-core/src/i18n/locales/ja/common.json";

void i18n.use(initReactI18next).init({
  lng: "ja",
  resources: { ja: { common } },
  initImmediate: false,
});

function Fixture() {
  const [result, setResult] = useState("pending");
  const [expired, setExpired] = useState(false);
  const questions = nativeInputQuestions({
    params: {
      questions: [
        {
          id: "__proto__",
          question: "同じ質問",
          header: "一つ目",
          options: [{ label: "Blue", description: "最初の選択肢" }],
        },
        {
          id: "constructor",
          question: "同じ質問",
          header: "二つ目",
          options: [{ label: "Green", description: "次の選択肢" }],
        },
      ],
    },
  })!;
  return (
    <main>
      <button onClick={() => setExpired(true)}>Expire</button>
      <AskUserQuestionBanner
        questions={questions}
        isSubmitting={false}
        isTimedOut={expired}
        error={null}
        onSubmitAnswers={(answers) => setResult(JSON.stringify({ answers }))}
      />
      <output>{result}</output>
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<Fixture />);

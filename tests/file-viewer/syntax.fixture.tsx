import { useState } from "react";
import { createRoot } from "react-dom/client";
import { CodeTextViewer } from "../../packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer";

function Fixture() {
  const [file, setFile] = useState({
    content: "",
    language: "",
    line: undefined as number | undefined,
  });
  Object.assign(window, {
    showViewer: (content: string, language: string, line?: number) =>
      setFile({ content, language, line }),
  });
  return (
    <main style={{ height: "100vh", minWidth: 0 }}>
      <CodeTextViewer {...file} />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

import { useState } from "react";
import { createRoot } from "react-dom/client";
import "../../packages/web-core/src/i18n/config";
import i18n from "../../packages/web-core/src/i18n/config";
import { MarkdownFileViewer } from "../../packages/web-core/src/features/workspace-files/ui/viewers/MarkdownFileViewer";
import { CodeTextViewer } from "../../packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer";

void i18n.changeLanguage("ja");
function Fixture() {
  const [file, setFile] = useState({ content: "", contentKey: "" });
  const [source, setSource] = useState(false);
  Object.assign(window, {
    showMarkdown: (content: string, contentKey: string) => {
      setSource(false);
      setFile({ content, contentKey });
    },
  });
  return (
    <main style={{ height: "100vh", minWidth: 0 }}>
      <button onClick={() => setSource(!source)}>Toggle source</button>
      {source ? (
        <CodeTextViewer content={file.content} language="markdown" />
      ) : (
        <MarkdownFileViewer {...file} />
      )}
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<Fixture />);

import { WebviewContextMenu } from "@/integrations/vscode/ContextMenu";
export function ContextMenuFixture() {
  return (
    <main className="min-h-screen bg-primary text-normal p-base">
      <textarea
        data-testid="editable"
        defaultValue="Raw 原文 abc"
        className="w-full bg-primary border border-border"
      />
      <textarea
        data-testid="readonly"
        defaultValue="Read only 原文"
        readOnly
        className="w-full bg-primary border border-border"
      />
      <p data-testid="raw-text">Raw model output 原文</p>
      <WebviewContextMenu />
    </main>
  );
}

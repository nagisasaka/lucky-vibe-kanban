export interface PromptPart {
  content: string;
  auxiliary: boolean;
}

// Display only: never normalize or rewrite the submitted/stored prompt.
export function splitPromptContext(content: string): PromptPart[] {
  const plain = () => [{ content, auxiliary: false }];
  const parts: PromptPart[] = [];
  const stack: string[] = [];
  let fence: { char: string; length: number } | undefined;
  let start = 0;
  let cursor = 0;
  let offset = 0;
  for (const line of content.match(/[^\n]*\n|[^\n]+$/g) ?? []) {
    const text = line.replace(/\r?\n$/, '');
    const codeFence = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(text);
    if (codeFence) {
      const [, marks, rest] = codeFence;
      if (!fence) fence = { char: marks[0], length: marks.length };
      else if (
        marks[0] === fence.char &&
        marks.length >= fence.length &&
        !rest.trim()
      )
        fence = undefined;
    } else if (!fence) {
      const marker =
        /^<!-- (lvk:card-context|lvk:shared-directories|vk:pipeline):(start|end) -->[ \t]*$/.exec(
          text
        );
      if (marker) {
        const [, kind, boundary] = marker;
        if (boundary === 'start') {
          if (stack.includes(kind)) return plain();
          if (!stack.length) start = offset;
          stack.push(kind);
        } else {
          // Ambiguous/incomplete markers stay visible rather than hiding user text.
          if (stack.pop() !== kind) return plain();
          if (!stack.length) {
            if (start > cursor)
              parts.push({
                content: content.slice(cursor, start),
                auxiliary: false,
              });
            cursor = offset + line.length;
            parts.push({
              content: content.slice(start, cursor),
              auxiliary: true,
            });
          }
        }
      }
    }
    offset += line.length;
  }
  if (stack.length || !parts.length) return plain();
  if (cursor < content.length)
    parts.push({ content: content.slice(cursor), auxiliary: false });
  return parts;
}

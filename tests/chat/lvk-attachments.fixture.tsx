import { createEditor } from "lexical";
import {
  $convertFromMarkdownString,
  $convertToMarkdownString,
} from "@lexical/markdown";
import { createAttachmentNode } from "../../packages/ui/src/components/attachment-node";
import { createImageNode } from "../../packages/ui/src/components/image-node";

const attachment = createAttachmentNode({});
const image = createImageNode({ openImagePreview: () => {} });
const transformers = [
  image.IMAGE_TRANSFORMER,
  attachment.ATTACHMENT_TRANSFORMER,
];
const editor = createEditor({
  namespace: "lvk-attachments",
  nodes: [attachment.AttachmentNode, image.ImageNode],
  onError: (error) => {
    throw error;
  },
});
(window as any).parseAttachments = (markdown: string) => {
  editor.update(() => $convertFromMarkdownString(markdown, transformers), {
    discrete: true,
  });
  return editor.getEditorState().read(() => ({
    markdown: $convertToMarkdownString(transformers),
    state: editor.getEditorState().toJSON(),
  }));
};

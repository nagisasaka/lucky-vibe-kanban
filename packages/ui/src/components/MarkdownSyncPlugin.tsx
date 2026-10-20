import { useEffect, useRef } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $convertToMarkdownString,
  $convertFromMarkdownString,
  type Transformer,
} from '@lexical/markdown';
import { $getRoot, type EditorState } from 'lexical';

type MarkdownSyncPluginProps = {
  value: string;
  onChange?: (markdown: string) => void;
  onEditorStateChange?: (state: EditorState) => void;
  editable: boolean;
  transformers: Transformer[];
};

/**
 * Handles bidirectional markdown synchronization between Lexical editor and external state.
 *
 * Uses an internal ref to prevent infinite update loops during bidirectional sync.
 */
export function MarkdownSyncPlugin({
  value,
  onChange,
  onEditorStateChange,
  editable,
  transformers,
}: MarkdownSyncPluginProps) {
  const [editor] = useLexicalComposerContext();
  const lastSerializedRef = useRef<string | undefined>(undefined);
  const lastExternalValueRef = useRef<string | undefined>(undefined);
  const prevTransformersRef = useRef(transformers);

  // Detect transformer changes and force re-parse
  if (transformers !== prevTransformersRef.current) {
    prevTransformersRef.current = transformers;
    lastSerializedRef.current = undefined;
    lastExternalValueRef.current = undefined;
  }

  // Handle editable state
  useEffect(() => {
    editor.setEditable(editable);
  }, [editor, editable]);

  // Handle controlled value changes (external → editor)
  useEffect(() => {
    if (value === lastExternalValueRef.current) return;
    lastExternalValueRef.current = value;
    if (value === lastSerializedRef.current) return;

    try {
      editor.update(
        () => {
          if (value.trim() === '') {
            $getRoot().clear();
          } else {
            $convertFromMarkdownString(value, transformers);
          }

          // Only position cursor at end if editor already has focus (user is actively editing)
          // This prevents unwanted focus when value changes externally (e.g., panel opening)
          const rootElement = editor.getRootElement();
          if (rootElement?.contains(document.activeElement)) {
            const root = $getRoot();
            const lastNode = root.getLastChild();
            if (lastNode) {
              lastNode.selectEnd();
            }
          }
        },
        { tag: 'markdown-sync' }
      );
    } catch (err) {
      console.error('Failed to parse markdown', err);
    }
  }, [editor, value, transformers]);

  // Handle editor changes (editor → external)
  useEffect(() => {
    return editor.registerUpdateListener(({ editorState, tags }) => {
      onEditorStateChange?.(editorState);
      if (!onChange) return;

      const markdown = editorState.read(() =>
        $convertToMarkdownString(transformers)
      );

      if (markdown === lastSerializedRef.current) return;

      lastSerializedRef.current = markdown;
      // Parsing controlled markdown may normalize trailing newlines. This is
      // not a user edit and must not invalidate an in-flight send snapshot.
      if (!tags.has('markdown-sync')) onChange(markdown);
    });
  }, [editor, onChange, onEditorStateChange, transformers]);

  return null;
}

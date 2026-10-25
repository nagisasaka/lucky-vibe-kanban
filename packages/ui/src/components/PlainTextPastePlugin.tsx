import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_HIGH,
  FORMAT_TEXT_COMMAND,
  PASTE_COMMAND,
} from 'lexical';

/** Ignore clipboard formatting and Lexical's internal rich clipboard payload. */
export function PlainTextPastePlugin() {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    const paste = editor.registerCommand(
      PASTE_COMMAND,
      (event) => {
        if (!(event instanceof ClipboardEvent) || !event.clipboardData)
          return false;
        // The host's capture handler owns files and attachments.
        if (event.clipboardData.files.length) return false;
        const text = event.clipboardData.getData('text/plain');
        const selection = $getSelection();
        if (!$isRangeSelection(selection)) return false;
        event.preventDefault();
        selection.setFormat(0);
        selection.insertRawText(text);
        return true;
      },
      COMMAND_PRIORITY_HIGH
    );
    const format = editor.registerCommand(
      FORMAT_TEXT_COMMAND,
      () => true,
      COMMAND_PRIORITY_HIGH
    );
    return () => {
      paste();
      format();
    };
  }, [editor]);
  return null;
}

import { useEffect } from 'react';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { $isLinkNode, LinkNode } from '@lexical/link';
import { $getNodeByKey, $nodesOfType } from 'lexical';

export interface ReadOnlyLinkPluginProps {
  resolveLocalLink?: (href: string) => (() => void) | undefined;
}

/** HTTPS keeps native navigation; local links only invoke a scoped viewer action. */
export function ReadOnlyLinkPlugin({
  resolveLocalLink,
}: ReadOnlyLinkPluginProps) {
  const [editor] = useLexicalComposerContext();

  useEffect(() => {
    const updateLink = (node: LinkNode) => {
      const dom = editor.getElementByKey(node.getKey());
      if (!(dom instanceof HTMLAnchorElement)) return;
      // Read the original Lexical URL: DOM href may already have been disabled
      // or sanitized, including when workspace metadata arrives after rendering.
      const href = node.getURL().trim();
      const external = /^https:\/\//i.test(href);
      const openLocal = external ? undefined : resolveLocalLink?.(href);
      dom.onclick = null;
      dom.onkeydown = null;
      dom.removeAttribute('target');
      dom.removeAttribute('rel');
      dom.removeAttribute('tabindex');
      dom.removeAttribute('aria-disabled');
      dom.setAttribute('role', 'link');
      dom.title = href;
      dom.style.pointerEvents = '';
      dom.style.cursor = external || openLocal ? 'pointer' : 'not-allowed';
      if (external) {
        dom.href = href;
        dom.target = '_blank';
        dom.rel = 'noopener noreferrer';
        dom.onclick = (event) => event.stopPropagation();
      } else {
        dom.removeAttribute('href');
        dom.onclick = (event) => {
          event.preventDefault();
          event.stopPropagation();
          openLocal?.();
        };
        if (openLocal) {
          dom.tabIndex = 0;
          dom.onkeydown = (event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              event.stopPropagation();
              openLocal();
            }
          };
        } else {
          dom.setAttribute('aria-disabled', 'true');
        }
      }
    };
    const unregister = editor.registerMutationListener(
      LinkNode,
      (mutations) => {
        editor.getEditorState().read(() => {
          for (const [key, mutation] of mutations) {
            if (mutation === 'destroyed') continue;
            const node = $getNodeByKey(key);
            if ($isLinkNode(node)) updateLink(node);
          }
        });
      }
    );
    editor
      .getEditorState()
      .read(() => $nodesOfType(LinkNode).forEach(updateLink));
    return unregister;
  }, [editor, resolveLocalLink]);

  return null;
}

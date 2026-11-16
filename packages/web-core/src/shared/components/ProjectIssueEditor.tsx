import {
  forwardRef,
  useImperativeHandle,
  useRef,
  type ComponentProps,
} from 'react';
import { useTranslation } from 'react-i18next';
import WYSIWYGEditor, { type WYSIWYGEditorRef } from './WYSIWYGEditor';
import { useImportedProject } from '@/shared/hooks/useImportedProject';
import { TransferArchiveText } from './TransferArchiveText';

/** Imported authored text remains literal, including during editing and lookup failures. */
export const ProjectIssueEditor = forwardRef<
  WYSIWYGEditorRef,
  ComponentProps<typeof WYSIWYGEditor> & { projectId: string }
>(function ProjectIssueEditor({ projectId, ...props }, ref) {
  const { keepTextInert } = useImportedProject(projectId);
  const editorRef = useRef<WYSIWYGEditorRef>(null);
  const textarea = useRef<HTMLTextAreaElement>(null);
  const { t } = useTranslation('common');
  useImperativeHandle(
    ref,
    () => ({
      focus: () =>
        keepTextInert ? textarea.current?.focus() : editorRef.current?.focus(),
    }),
    [keepTextInert]
  );
  if (!keepTextInert) return <WYSIWYGEditor {...props} ref={editorRef} />;
  return (
    <div className={props.className}>
      {props.disabled ? (
        <>
          <TransferArchiveText content={props.value} />
          {props.onRequestEdit && (
            <button
              type="button"
              className="text-brand underline"
              onClick={props.onRequestEdit}
            >
              {t('markdownEditor.edit')}
            </button>
          )}
        </>
      ) : (
        <textarea
          ref={textarea}
          aria-label={
            props.placeholder || t('kanban.issueDescriptionPlaceholder')
          }
          className="block w-full min-h-[100px] whitespace-pre-wrap rounded-sm border bg-primary p-base text-normal"
          value={props.value}
          placeholder={props.placeholder}
          autoFocus={props.autoFocus}
          onChange={(e) => props.onChange?.(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              if (e.shiftKey) props.onShiftCmdEnter?.();
              else props.onCmdEnter?.();
            }
          }}
        />
      )}
    </div>
  );
});

import type { BuildInfo } from 'shared/types';
import { useTranslation } from 'react-i18next';
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
  PopoverClose,
} from './Popover';

export function BuildInfoLabel({
  buildInfo,
}: {
  buildInfo?: BuildInfo | null;
}) {
  const { t } = useTranslation('common');
  if (
    !buildInfo ||
    !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/i.test(buildInfo.revision)
  ) {
    return null;
  }

  const kind =
    buildInfo.kind === 'release' && buildInfo.release_tag
      ? `${t('buildInfo.release')} (${buildInfo.release_tag})`
      : buildInfo.kind === 'development'
        ? t('buildInfo.development')
        : t('buildInfo.unknownKind');
  const dirty =
    buildInfo.dirty === true
      ? t('buildInfo.dirty')
      : buildInfo.dirty === false
        ? t('buildInfo.clean')
        : t('buildInfo.unknownDirty');
  const details = `${kind}\n${buildInfo.revision}\n${dirty}`;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          data-testid="app-build-info"
          className="text-[9px] font-ibm-plex-mono text-low leading-none truncate w-10 min-h-10 text-center rounded-sm hover:text-normal hover:bg-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand"
          aria-label={`${t('buildInfo.details')}: ${buildInfo.revision.slice(0, 7)}`}
        >
          {buildInfo.revision.slice(0, 7)}
        </button>
      </PopoverTrigger>
      <PopoverContent
        side="top"
        align="start"
        collisionPadding={8}
        className="max-w-[calc(100vw-1rem)] text-sm text-normal"
        aria-label={t('buildInfo.details')}
      >
        <p className="whitespace-pre-line break-all font-ibm-plex-mono">
          {details}
        </p>
        <PopoverClose asChild>
          <button
            type="button"
            className="mt-base min-h-10 px-base rounded-sm hover:bg-secondary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand"
          >
            {t('buttons.close')}
          </button>
        </PopoverClose>
      </PopoverContent>
    </Popover>
  );
}

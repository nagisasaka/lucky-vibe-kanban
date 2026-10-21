import type { BuildInfo } from 'shared/types';
import { useTranslation } from 'react-i18next';

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
    <p
      data-testid="app-build-info"
      className="text-[9px] font-ibm-plex-mono text-low leading-none truncate max-w-10 text-center"
      title={details}
      aria-label={details}
    >
      {buildInfo.revision.slice(0, 7)}
    </p>
  );
}

import { lazy, Suspense } from 'react';

// Keep the package's dynamic import unreachable in production builds.
const Agentation = import.meta.env.DEV
  ? lazy(() =>
      import('agentation').then((module) => ({ default: module.Agentation }))
    )
  : null;

/** Internal preview trial: local annotations and manual clipboard export only. */
export function DevAgentation() {
  if (
    !Agentation ||
    window.location.hostname !== 'preview2.luckyvibekanban.com'
  ) {
    return null;
  }
  return (
    <Suspense fallback={null}>
      <Agentation />
    </Suspense>
  );
}

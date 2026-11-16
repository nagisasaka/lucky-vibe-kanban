// Production ThemeProvider, colour theme picker and UI primitives with the
// shipped CSS; the saved config is local fixture state.
import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { DarkColorTheme, LightColorTheme, ThemeMode } from 'shared/types';
import settings from '../../packages/web-core/src/i18n/locales/ja/settings.json';
import { ThemeProvider } from '../../packages/local-web/src/app/providers/ThemeProvider';
import { ColorThemeSettings } from '../../packages/web-core/src/shared/dialogs/settings/settings/ColorThemeSettings';
import { PrimaryButton } from '../../packages/ui/src/components/PrimaryButton';

i18n
  .use(initReactI18next)
  .init({ lng: 'ja', resources: { ja: { settings } }, initImmediate: false });

type Saved = {
  theme: ThemeMode;
  light: LightColorTheme;
  dark: DarkColorTheme;
};

/** Representative surfaces of the board, Issue and Workspace screens. */
function Sample() {
  return (
    <div
      className="flex flex-col border border-border text-normal sm:flex-row"
      id="sample"
    >
      <aside className="shrink-0 space-y-1 bg-secondary p-base text-sm sm:w-40">
        <p className="font-semibold text-high">Lucky Vibe Kanban</p>
        <p className="rounded-sm bg-brand/20 px-1 text-high">ボード</p>
        <p className="rounded-sm bg-panel px-1">Workspace（hover）</p>
        <p className="text-low">補助テキスト</p>
      </aside>
      <main className="flex-1 space-y-base bg-primary p-base text-sm">
        <div className="flex flex-wrap items-center gap-base">
          <span className="font-semibold text-high">In Progress</span>
          <PrimaryButton value="新規 Issue" />
          <PrimaryButton variant="secondary" value="開始" />
          <button className="rounded-sm bg-error px-base py-half text-on-error">
            削除
          </button>
          <button className="rounded-sm bg-success px-base py-half text-on-success">
            承認
          </button>
        </div>
        <div className="grid gap-base sm:grid-cols-2">
          <div className="space-y-1 rounded-sm border border-border bg-panel p-base">
            <p className="text-high">LOCAL-382 カラーテーマ</p>
            <p className="text-low">選択テキストと補助テキスト</p>
            <p>
              <span className="text-success">● 成功</span>{' '}
              <span className="text-error">● 失敗</span>{' '}
              <span className="text-merged">● マージ済み</span>{' '}
              <span style={{ color: 'hsl(var(--warning))' }}>● 警告</span>{' '}
              <span style={{ color: 'hsl(var(--info))' }}>● 情報</span>
            </p>
          </div>
          <div className="space-y-1 rounded-sm border border-brand bg-brand/10 p-base">
            <p className="text-high">選択中のカード</p>
            <p className="text-low">bg-brand/10 の上の補助テキスト</p>
            <a className="text-brand underline" href="#sample">
              text-brand のリンク
            </a>
          </div>
        </div>
        <div className="markdown-body">
          <p>
            Issue 本文の <a href="#sample">リンク</a> と <code>code</code>
          </p>
        </div>
        <div className="overflow-hidden rounded-sm border border-border font-ibm-plex-mono text-xs">
          <div style={{ background: 'hsl(var(--line-added))' }}>
            + const theme = &apos;navy&apos;;
          </div>
          <div style={{ background: 'hsl(var(--line-removed))' }}>
            - const theme = &apos;dark&apos;;
          </div>
        </div>
        <pre
          className="rounded-sm p-base text-xs"
          style={{
            background: 'hsl(var(--console-background))',
            color: 'hsl(var(--console-foreground))',
          }}
        >
          $ pnpm run check
        </pre>
      </main>
    </div>
  );
}

function Fixture() {
  const [saved, setSaved] = useState<Saved>({
    theme: ThemeMode.SYSTEM,
    light: LightColorTheme.STANDARD,
    dark: DarkColorTheme.STANDARD,
  });
  const [draft, setDraft] = useState<Saved>(saved);
  Object.assign(window, {
    savedTheme: () => saved,
    setSavedTheme: (value: Saved) => {
      setSaved(value);
      setDraft(value);
    },
  });
  return (
    <ThemeProvider
      initialTheme={saved.theme}
      initialLightColorTheme={saved.light}
      initialDarkColorTheme={saved.dark}
    >
      <div className="min-h-screen space-y-double bg-primary p-double text-normal">
        <section className="max-w-xl space-y-base" aria-label="settings">
          <ColorThemeSettings
            theme={draft.theme}
            lightColorTheme={draft.light}
            darkColorTheme={draft.dark}
            onThemeChange={(theme) => setDraft({ ...draft, theme })}
            onLightColorThemeChange={(light) => setDraft({ ...draft, light })}
            onDarkColorThemeChange={(dark) => setDraft({ ...draft, dark })}
          />
          <PrimaryButton value="保存" onClick={() => setSaved(draft)} />
        </section>
        <Sample />
      </div>
    </ThemeProvider>
  );
}

createRoot(document.getElementById('root')!).render(<Fixture />);

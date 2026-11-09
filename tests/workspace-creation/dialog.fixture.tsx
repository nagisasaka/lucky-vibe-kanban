import React from 'react';
import { createRoot } from 'react-dom/client';
import NiceModal from '@ebay/nice-modal-react';
import { HotkeysProvider } from 'react-hotkeys-hook';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import common from '../../packages/web-core/src/i18n/locales/ja/common.json';
import { WorkspaceTargetDialog } from '../../packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog';
void i18n.use(initReactI18next).init({
  lng: 'ja',
  resources: { ja: { common } },
  interpolation: { escapeValue: false },
});
function Fixture() {
  const [result, setResult] = React.useState('');
  return (
    <>
      <button
        onClick={() =>
          void WorkspaceTargetDialog.show({ projectId: 'project' }).then((v) =>
            setResult(JSON.stringify(v))
          )
        }
      >
        Project new
      </button>
      <button
        onClick={() =>
          void WorkspaceTargetDialog.show({}).then((v) =>
            setResult(JSON.stringify(v))
          )
        }
      >
        Orphan new
      </button>
      <button
        onClick={() =>
          void WorkspaceTargetDialog.show({ purpose: 'project_default' }).then(
            (v) => setResult(JSON.stringify(v))
          )
        }
      >
        Preset
      </button>
      <pre id="result">{result}</pre>
    </>
  );
}
createRoot(document.getElementById('root')!).render(
  <HotkeysProvider>
    <NiceModal.Provider>
      <Fixture />
    </NiceModal.Provider>
  </HotkeysProvider>
);

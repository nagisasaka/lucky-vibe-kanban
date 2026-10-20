import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import { useExecutorConfig } from '../../packages/web-core/src/shared/hooks/useExecutorConfig';
import { BaseCodingAgent, type ExecutorConfig } from '../../shared/types';

const saved: Record<string, ExecutorConfig | undefined> = {
  a: { executor: BaseCodingAgent.CODEX, reasoning_id: 'high' },
};
const launch: ExecutorConfig = {
  executor: BaseCodingAgent.CODEX,
  model_id: 'test-model',
  reasoning_id: 'max',
};
function Composer({ scope }: { scope: string }) {
  const [scratch, setScratch] = useState(saved[scope]);
  const { executorConfig, setOverrides } = useExecutorConfig({
    profiles: { CODEX: { configurations: { DEFAULT: { CODEX: {} } } } } as never,
    lastUsedConfig: scope === 'a' ? launch : { executor: BaseCodingAgent.CODEX },
    scratchConfig: scratch,
    onPersist: (config) => { saved[scope] = config; setScratch(config); },
  });
  return <>
    <output id="config">{JSON.stringify(executorConfig)}</output>
    <output id="saved">{JSON.stringify(scratch)}</output>
    <button onClick={() => setOverrides({ reasoning_id: 'ultra' })}>Ultra</button>
    <button onClick={() => setOverrides({ reasoning_id: null })}>Follow Settings</button>
    <button onClick={() => setOverrides({ agent_id: 'test-agent' })}>Other override</button>
    <button onClick={() => setOverrides({ model_id: 'other-model' })}>Change model</button>
  </>;
}
function App() {
  const [scope, setScope] = useState('a');
  const [mount, setMount] = useState(0);
  return <>
    <button onClick={() => setScope(scope === 'a' ? 'b' : 'a')}>Switch workspace</button>
    <button onClick={() => setMount(mount + 1)}>Remount</button>
    <Composer key={`${scope}:${mount}`} scope={scope} />
  </>;
}
createRoot(document.getElementById('root')!).render(<App />);

import { createRoot } from 'react-dom/client';
import { DevAgentation } from '../../packages/local-web/src/app/entry/DevAgentation';

createRoot(document.getElementById('root')!).render(
  <>
    <main style={{ padding: 40 }}>
      <h1>Isolated design review</h1>
      <button className="quota-fixture" style={{ padding: 20 }}>
        Quota fixture
      </button>
    </main>
    <DevAgentation />
  </>
);

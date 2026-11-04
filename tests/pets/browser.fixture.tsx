import { createRoot } from 'react-dom/client';
import { useState } from 'react';
import i18n from '../../packages/web-core/src/i18n/config';
import { PetSettings } from '../../packages/web-core/src/features/pets/SidebarPet';
import { PetSprite } from '../../packages/web-core/src/features/pets/PetSprite';
import { PET_STATES } from '../../packages/web-core/src/features/pets/format';
void i18n.changeLanguage('en');
function App() {
  const [shown, setShown] = useState(true);
  return (
    <main className="bg-primary text-normal p-double min-h-screen">
      <h1>Isolated pet component verification</h1>
      <button onClick={() => setShown(!shown)}>Toggle sidebar</button>
      {shown && (
        <section
          className="relative p-double border"
          style={{ width: 320, height: 480 }}
        >
          <PetSettings />
        </section>
      )}
      <section style={{ display: 'flex', gap: 24 }}>
        {PET_STATES.map((s) => (
          <div key={s.name}>
            <p>{s.name}</p>
            <PetSprite
              src="/pets/lvk-rabbit/spritesheet.webp"
              name={s.name}
              state={s.name}
            />
          </div>
        ))}
      </section>
    </main>
  );
}
createRoot(document.getElementById('root')!).render(<App />);

import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { GuideDialogShell } from '@vibe/ui/components/GuideDialogShell';
import { InteractiveHelp } from '@/shared/components/InteractiveHelp';
import { ThemeProviderContext } from '@/shared/hooks/useTheme';
import { ThemeMode } from 'shared/types';
import i18n from '@/i18n/config';

void i18n.changeLanguage('en').then(() => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  createRoot(document.getElementById('root')!).render(
    <ThemeProviderContext.Provider
      value={{ theme: ThemeMode.LIGHT, setTheme: () => {} }}
    >
      <QueryClientProvider client={client}>
        <GuideDialogShell
          topics={[
            { id: 'help', title: 'Ask LVK', content: <InteractiveHelp /> },
          ]}
          closeLabel="Close"
          onClose={() => {}}
        />
      </QueryClientProvider>
    </ThemeProviderContext.Provider>
  );
});

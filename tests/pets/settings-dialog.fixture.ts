export const SettingsDialog = {
  show: async (options: unknown) => {
    document.documentElement.dataset.openedSettings = JSON.stringify(options);
  },
};

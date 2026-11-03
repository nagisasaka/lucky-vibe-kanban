export const attachmentsApi = {
  upload: async (image: File) => {
    await fetch("/fixture-upload", { method: "POST", body: image });
    return { id: "11111111-1111-4111-8111-111111111111" };
  },
  getAttachmentUrl: () => localStorage.getItem("pet-image-failure") ? "/missing-import.webp" : "/pets/lvk-rabbit/spritesheet.webp",
};

export async function persistPetPreferences() {
  if (localStorage.getItem('pet-save-failure')) throw new Error('Fixture save failure');
}

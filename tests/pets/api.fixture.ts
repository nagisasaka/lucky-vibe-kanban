export const attachmentsApi = {
  upload: async (image: File) => {
    const response = await fetch("/fixture-upload", { method: "POST", body: image });
    if (!response.ok) throw new Error("Fixture upload failure");
    const data = await new Promise<string>((resolve) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.readAsDataURL(image);
    });
    localStorage.setItem("pet-fixture-image", data);
    return { id: "11111111-1111-4111-8111-111111111111" };
  },
  getAttachmentUrl: () =>
    localStorage.getItem("pet-image-failure")
      ? "/missing-import.webp"
      : localStorage.getItem("pet-fixture-image") ||
        "/pets/lvk-rabbit/spritesheet.webp",
};

export async function persistPetPreferences() {
  if (localStorage.getItem("pet-save-failure"))
    throw new Error("Fixture save failure");
}

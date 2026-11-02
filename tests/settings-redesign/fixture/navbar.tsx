import { Navbar } from "@vibe/ui/components/Navbar";
export function NavbarFixture() {
  return (
    <Navbar
      leftItems={[]}
      rightItems={[]}
      syncErrors={[
        {
          streamId: "raw-stream",
          tableName: "raw-table",
          error: { message: "Raw internal secret", status: 503 },
        },
      ]}
    />
  );
}

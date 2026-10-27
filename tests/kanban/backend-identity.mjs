import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { execFileSync } from "node:child_process";

export async function verifyBackendIdentity(binary, info, root) {
  assert.match(
    info.build_info?.revision ?? "",
    /^[a-f0-9]{40}$/,
    "backend must report a full source SHA",
  );
  // Test-only frontend changes may be dirty; all compiled inputs must match.
  execFileSync(
    "git",
    [
      "diff",
      "--exit-code",
      info.build_info.revision,
      "--",
      "crates",
      "Cargo.toml",
      "Cargo.lock",
    ],
    { cwd: root, stdio: "pipe" },
  );
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(binary)) hash.update(chunk);
  return { binarySha256: hash.digest("hex"), build: info.build_info };
}

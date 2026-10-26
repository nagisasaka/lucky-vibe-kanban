#!/usr/bin/env node
// Cargo invokes this only for compiler processes, never for `cargo run`'s binary.
import { existsSync, realpathSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const shared = join(dirname(fileURLToPath(import.meta.url)), '../.lvk-shared/cache');
const env = { ...process.env };
if (existsSync(shared)) env.SCCACHE_DIR = join(realpathSync(shared), 'sccache');
const result = spawnSync('sccache', process.argv.slice(2), { env, stdio: 'inherit' });
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);

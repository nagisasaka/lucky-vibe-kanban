# Header / AppBar regression

From this checkout, build and run:

```sh
pnpm --dir packages/local-web run build
cargo build -j 1 --bin server --bin agent-process-host
LVK_CHROMIUM_EXECUTABLE=/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome node tests/header/isolated.mjs
```

Run these commands sequentially; do not overlap builds, checks, or E2E.
The browser location is environment-specific; `LVK_CHROMIUM_EXECUTABLE` also
accepts an installed Chromium executable. The script uses this checkout's
`target/debug/server`, built frontend, a fresh dedicated `LVK_INSTANCE_DIR`,
local authentication, and OS-assigned loopback ports. It closes Chromium, stops
and waits for its server, and removes its temporary instance in `finally`. The server has a managed
process group so CLI capability-probe children also receive termination.
It never connects to the running development instance.

Real backend operations create a project and independent workspace in SQLite,
configure onboarding, and exercise both routes and their sidebar navigation.
Only GitHub's external API is intercepted: zero, 1,234 stars, HTTP 403, negative
count, missing count, and network failure; delayed loading also must not show zero prematurely.
Expected badges are `0`, `1.2k`, and
no badge for failures/invalid data. Assertions require the official LVK link,
absence of Feedback/Discord/standalone Local, no obsolete external requests,
and retained project/create-project/workspaces navigation. The zero case uses
dark theme, subsequent cases use light theme. The command bar must also omit
Feedback. At 390px width, the project drawer must navigate to Workspaces and
back. The local ConfigProvider always bypasses onboarding: its redirect to
the first project is tested, while the unreachable LandingPage social changes are
checked statically, not claimed as browser coverage.

Results, cleanup receipt (`cleanup.json`), screenshots and server/build logs are retained under ignored
`.build/header-validation/`. Mock results do not prove live GitHub availability.

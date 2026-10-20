# Header / AppBar regression

From this checkout, build and run:

```sh
pnpm --dir packages/local-web run build
cargo build --bin server --bin agent-process-host
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/header/isolated.mjs
```

The browser location is environment-specific; `LVK_CHROMIUM_EXECUTABLE` also
accepts an installed Chromium executable. The script uses this checkout's
`target/debug/server`, built frontend, a fresh dedicated `LVK_INSTANCE_DIR`,
local authentication, and OS-assigned loopback ports. It closes Chromium, stops
and waits for its server, and removes its temporary instance in `finally`.
It never connects to the running development instance.

Real backend operations create a project and independent workspace in SQLite,
configure onboarding, and exercise both routes and their sidebar navigation.
GitHub's external API is intercepted (the onboarding destination page is also
stubbed while asserting the actual popup URL): zero, 1,234 stars, HTTP 403, negative
count, missing count, and network failure; delayed loading also must not show zero prematurely.
Expected badges are `0`, `1.2k`, and
no badge for failures/invalid data. Assertions require the official LVK link,
absence of Feedback/Discord/standalone Local, no obsolete external requests,
and retained project/create-project/workspaces navigation. The zero case uses
dark theme, subsequent cases use light theme. The command bar must also omit
Feedback, and onboarding must open LVK GitHub without a Discord button.

Results, screenshots and server/build logs are retained under ignored
`.build/header-validation/`. Mock results do not prove live GitHub availability.

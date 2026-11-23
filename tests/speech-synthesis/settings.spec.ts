import { test, expect, type Page } from "@playwright/test";

async function mockSpeech(page: Page) {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "languages", {
      value: ["ja-JP", "en-US"],
      configurable: true,
    });
    Object.defineProperty(navigator, "language", {
      value: "ja-JP",
      configurable: true,
    });
    (window as any).SpeechSynthesisUtterance = class {
      voice = null;
      rate = 1;
      lang = "";
      constructor(public text: string) {}
    };
    const synthesis = new EventTarget();
    Object.assign(window, {
      speechCalls: [],
      speechCancels: 0,
      testVoices: [],
    });
    Object.assign(synthesis, {
      getVoices: () => (window as any).testVoices,
      speak: (utterance: any) => (window as any).speechCalls.push(utterance),
      cancel: () => (window as any).speechCancels++,
    });
    Object.defineProperty(window, "speechSynthesis", {
      value: synthesis,
      configurable: true,
    });
  });
}
const voice = {
  voiceURI: "fixture:ja",
  name: "Fixture Japanese",
  lang: "ja-JP",
};
const voiceId = JSON.stringify([voice.voiceURI, voice.name, voice.lang]);
async function setVoices(
  page: Page,
  voices = [
    voice,
    { ...voice, name: "Second Japanese", voiceURI: "fixture:second" },
  ],
) {
  await page.evaluate((voices) => {
    (window as any).testVoices = voices;
    window.speechSynthesis.dispatchEvent(new Event("voiceschanged"));
  }, voices);
}

test("defaults, delayed voices, save/reload, missing voice and preview without enabling board", async ({
  page,
}) => {
  await mockSpeech(page);
  await page.goto("/");
  await expect(
    page.getByRole("combobox", { name: "Voice", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("combobox", { name: "Speaking rate" }),
  ).toHaveValue("1");
  await setVoices(page);
  await expect(
    page.getByRole("option", { name: "Fixture Japanese (ja-JP)" }),
  ).toHaveCount(1);
  await page
    .getByRole("combobox", { name: "Voice", exact: true })
    .selectOption(voiceId);
  await page
    .getByRole("combobox", { name: "Speaking rate" })
    .selectOption("1.7");
  await page.reload();
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem("speech-synthesis-preferences")!).voice,
    ),
  ).toBe(voiceId);
  await expect(
    page.getByText("The saved voice is unavailable.", { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Speaking rate" }),
  ).toHaveValue("1.7");
  await page.getByRole("button", { name: "Preview", exact: true }).click();
  expect(
    await page.evaluate(() => {
      const utterance = (window as any).speechCalls[0];
      const result = {
        rate: Number(utterance.rate.toFixed(1)),
        voice: utterance.voice,
        cancels: (window as any).speechCancels,
      };
      utterance.onend();
      return result;
    }),
  ).toEqual({ rate: 1.7, voice: null, cancels: 0 });
  await expect(
    page.getByRole("button", { name: "Turn on Run speech" }),
  ).toHaveAttribute("aria-pressed", "false");
  await setVoices(page);
  await expect(
    page.getByText("The saved voice is unavailable.", { exact: false }),
  ).toHaveCount(0);
  await setVoices(page, []);
  expect(
    await page.evaluate(
      () =>
        JSON.parse(localStorage.getItem("speech-synthesis-preferences")!).voice,
    ),
  ).toBe(voiceId);
  await page.getByRole("button", { name: "Toggle settings" }).click();
  expect(await page.evaluate(() => (window as any).speechCancels)).toBe(0);
});

test("another tab updates choices; preview and settings close preserve board utterance and queue", async ({
  page,
  context,
}) => {
  await mockSpeech(page);
  let requests = 0;
  await page.route(
    "**/api/projects/fixture/run-observations*",
    async (route) => {
      requests++;
      await route.fulfill({
        json: {
          cursor: requests,
          active: ["one", "two"],
          pending: [],
          events:
            requests === 2
              ? ["one", "two"].map((run_id) => ({
                  run_id,
                  issue: run_id,
                  attempt: 1,
                  payload: {
                    type: "lifecycle_changed",
                    data: { status: "starting" },
                  },
                }))
              : [],
        },
      });
    },
  );
  await page.goto("/");
  const other = await context.newPage();
  await mockSpeech(other);
  await other.goto("/");
  await other
    .getByRole("combobox", { name: "Speaking rate" })
    .selectOption("1.4");
  await expect(
    page.getByRole("combobox", { name: "Speaking rate" }),
  ).toHaveValue("1.4");
  await page.getByRole("button", { name: "Turn on Run speech" }).click();
  await expect
    .poll(() => page.evaluate(() => (window as any).speechCalls.length))
    .toBe(1);
  await page.getByRole("button", { name: "Preview", exact: true }).click();
  await page.getByRole("button", { name: "Toggle settings" }).click();
  expect(
    await page.evaluate(() => ({
      count: (window as any).speechCalls.length,
      cancels: (window as any).speechCancels,
    })),
  ).toEqual({ count: 2, cancels: 0 });
  await other
    .getByRole("combobox", { name: "Speaking rate" })
    .selectOption("0.8");
  await page.evaluate(() => (window as any).speechCalls[0].onend());
  await expect
    .poll(() => page.evaluate(() => (window as any).speechCalls.length))
    .toBe(3);
  expect(
    await page.evaluate(() =>
      (window as any).speechCalls.map((u: any) => Number(u.rate.toFixed(1))),
    ),
  ).toEqual([1.4, 1.4, 0.8]);
  await page.getByRole("button", { name: "Turn off Run speech" }).click();
  expect(
    await page.evaluate(() => (window as any).speechCancels),
  ).toBeGreaterThan(0);
});

test("unsupported browser and Japanese mobile dark layout", async ({
  page,
}) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, "speechSynthesis", { value: undefined });
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?lang=ja");
  await page.evaluate(() => document.documentElement.classList.add("dark"));
  await expect(page.locator("html")).toHaveClass("dark");
  await expect(page.getByRole("heading", { name: "音声合成" })).toBeVisible();
  await expect(
    page.getByText("このブラウザは音声合成に対応していません。"),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "試聴", exact: true }),
  ).toBeDisabled();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "test-results/speech-synthesis/mobile-dark.png",
  });
});

test("desktop light layout and native API availability (no audible assertion)", async ({
  page,
}) => {
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Speech Synthesis" }),
  ).toBeVisible();
  console.log(
    "Native speech capability:",
    await page.evaluate(() => ({
      supported: "speechSynthesis" in window,
      voices: window.speechSynthesis?.getVoices().length,
    })),
  );
  await page.screenshot({
    path: "test-results/speech-synthesis/desktop-light.png",
  });
});

test("content mode defaults, persists and leaves preview available", async ({
  page,
}) => {
  await mockSpeech(page);
  await page.goto("/");
  const mode = page.getByRole("combobox", { name: "What to read aloud" });
  await expect(mode).toHaveValue("all");
  await mode.selectOption("results-and-action");
  await page.reload();
  await expect(mode).toHaveValue("results-and-action");
  await page.getByRole("button", { name: "Preview", exact: true }).click();
  expect(await page.evaluate(() => (window as any).speechCalls.length)).toBe(1);
  await expect(
    page.getByRole("button", { name: "Turn on Run speech" }),
  ).toHaveAttribute("aria-pressed", "false");
  await page.goto("/?lang=ja");
  await expect(
    page.getByRole("combobox", { name: "読み上げる内容" }),
  ).toHaveValue("results-and-action");
  await expect(
    page.getByRole("option", { name: "結果と要対応だけ", exact: true }),
  ).toHaveCount(1);
});

test("provider reads live content mode and never replays excluded events", async ({ page }) => {
  await mockSpeech(page);
  await page.addInitScript(() => {
    (window as any).SpeechSynthesisUtterance = class {
      text: string;
      constructor(text: string) { this.text = text; }
    };
  });
  let cursor = 0;
  const events: any[] = [];
  await page.route("**/api/projects/fixture/run-observations*", route => {
    const after = new URL(route.request().url()).searchParams.get("after");
    return route.fulfill({ json: { cursor, active: ["first", "excluded", "new"], pending: [],
      events: after === null ? [] : events.filter(e => e.cursor > Number(after)) } });
  });
  const add = (run_id: string) => events.push({ cursor: ++cursor, run_id, issue: run_id, attempt: 1,
    payload: { type: "lifecycle_changed", data: { status: "starting" } } });
  await page.goto("/");
  await setVoices(page);
  await page.getByRole("combobox", { name: "Voice", exact: true }).selectOption(voiceId);
  await page.getByRole("combobox", { name: "Speaking rate" }).selectOption("1.7");
  await page.getByRole("button", { name: "Turn on Run speech" }).click();
  await expect(page.getByRole("button", { name: "Turn off Run speech" })).toHaveAttribute("aria-pressed", "true");
  add("first");
  await expect.poll(() => page.evaluate(() => (window as any).speechCalls.length)).toBe(1);
  expect(await page.evaluate(() => ({ rate: (window as any).speechCalls[0].rate, voice: (window as any).speechCalls[0].voice.name }))).toEqual({ rate: 1.7, voice: voice.name });
  await page.getByRole("combobox", { name: "What to read aloud" }).selectOption("results-and-action");
  add("excluded");
  await page.waitForTimeout(1200);
  await page.evaluate(() => (window as any).speechCalls[0].onend());
  await page.waitForTimeout(1200);
  expect(await page.evaluate(() => (window as any).speechCalls.length)).toBe(1);
  await page.getByRole("combobox", { name: "What to read aloud" }).selectOption("all");
  await page.waitForTimeout(1200);
  expect(await page.evaluate(() => (window as any).speechCalls.length)).toBe(1);
  add("new");
  await expect.poll(() => page.evaluate(() => (window as any).speechCalls.length)).toBe(2);
});

for (const scenario of [
  {
    name: "Windows multiple providers",
    voices: [
      { voiceURI: "g", name: "Google 日本語", lang: "ja-JP" },
      { voiceURI: "m", name: "Microsoft Haruka", lang: "ja_JP" },
      { voiceURI: "third", name: "Third-party Japanese", lang: "ja-JP" },
      { voiceURI: "en", name: "English voice", lang: "en-US" },
    ],
    selected: 2,
  },
  {
    name: "Android locale-only single voice",
    voices: [{ voiceURI: "ja-jp", name: "日本語 日本", lang: "ja_JP" }],
    selected: 0,
  },
]) {
  test(`${scenario.name}: named candidates and identical preview/board voice`, async ({
    page,
  }) => {
    await mockSpeech(page);
    let requests = 0;
    await page.route("**/api/projects/fixture/run-observations*", (route) => {
      requests++;
      return route.fulfill({
        json: {
          cursor: requests,
          active: [],
          pending: [],
          events:
            requests === 2
              ? [
                  {
                    run_id: "done",
                    issue: "LOCAL-1",
                    attempt: 1,
                    payload: {
                      type: "lifecycle_changed",
                      data: { status: "succeeded" },
                    },
                  },
                ]
              : [],
        },
      });
    });
    await page.goto("/");
    await expect(
      page.getByRole("combobox", { name: "Voice", exact: true }),
    ).toHaveCount(0);
    await setVoices(page, scenario.voices);
    const chosen = scenario.voices[scenario.selected];
    const id = JSON.stringify([chosen.voiceURI, chosen.name, chosen.lang]);
    if (scenario.voices.length === 1) {
      await expect(
        page.getByText("日本語 日本 (ja_JP)", { exact: true }),
      ).toBeVisible();
      await expect(
        page.getByRole("combobox", { name: "Voice", exact: true }),
      ).toHaveCount(0);
      await page.setViewportSize({ width: 390, height: 844 });
      await page.evaluate(() => document.documentElement.classList.add("dark"));
    } else {
      await expect(
        page.getByRole("option", {
          name: "English voice (en-US)",
          exact: true,
        }),
      ).toHaveCount(0);
      for (const voice of scenario.voices.slice(0, 3)) {
        await expect(
          page.getByRole("option", {
            name: `${voice.name} (${voice.lang})`,
            exact: true,
          }),
        ).toHaveCount(1);
      }
      await page
        .getByRole("combobox", { name: "Voice", exact: true })
        .selectOption(id);
      await page.reload();
      await setVoices(page, scenario.voices);
      await expect(
        page.getByRole("combobox", { name: "Voice", exact: true }),
      ).toHaveValue(id);
    }
    await page.screenshot({
      path: `test-results/speech-synthesis/${scenario.selected === 0 ? "single-mobile-dark" : "multiple-desktop"}.png`,
    });
    await page.getByRole("button", { name: "Preview", exact: true }).click();
    await page.evaluate(() => (window as any).speechCalls[0].onend());
    await page.getByRole("button", { name: "Turn on Run speech" }).click();
    await expect
      .poll(() => page.evaluate(() => (window as any).speechCalls.length))
      .toBe(2);
    expect(
      await page.evaluate(() =>
        (window as any).speechCalls.map((u: any) => ({
          name: u.voice?.name,
          lang: u.lang,
        })),
      ),
    ).toEqual([
      { name: chosen.name, lang: "ja-JP" },
      { name: chosen.name, lang: "ja-JP" },
    ]);
    await page.getByRole("button", { name: "Turn off Run speech" }).click();
    if (scenario.voices.length > 1) {
      await setVoices(page, scenario.voices.slice(0, 2));
      await page.getByRole("button", { name: "Preview", exact: true }).click();
      expect(
        await page.evaluate(() => (window as any).speechCalls.at(-1).voice),
      ).toBeNull();
      expect(
        await page.evaluate(
          () =>
            JSON.parse(localStorage.getItem("speech-synthesis-preferences")!)
              .voice,
        ),
      ).toBe(id);
    } else {
      await page.evaluate(() => {
        Object.defineProperty(navigator, "languages", {
          value: ["en-US", "ja-JP"],
          configurable: true,
        });
        window.dispatchEvent(new Event("languagechange"));
      });
      await expect(
        page.getByText("No voices for your preferred language", {
          exact: false,
        }),
      ).toBeVisible();
      await expect(
        page.getByText("日本語 日本 (ja_JP)", { exact: true }),
      ).toHaveCount(0);
    }
  });
}
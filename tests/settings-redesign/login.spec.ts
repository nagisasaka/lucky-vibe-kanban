import { test, expect } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
const directory = '.build/design-preview/login-redesign';
for (const width of [360, 768, 1440])
  for (const lang of ['en', 'ja'])
    for (const theme of ['light', 'dark']) {
      test(`login ${width} ${lang} ${theme}: Basic form and successful fixture login`, async ({
        page,
      }) => {
        await page.setViewportSize({ width, height: 900 });
        await page.goto(`/?view=login&auth=basic&lang=${lang}&theme=${theme}`);
        await expect(
          page.getByRole('heading', {
            name:
              lang === 'ja'
                ? 'LVK ログイン・初期設定'
                : 'LVK Sign in and initial setup',
          })
        ).toBeVisible();
        await expect(
          page.getByText('Lucky Vibe Kanban', { exact: true })
        ).toBeVisible();
        const user = page.getByLabel(
          lang === 'ja' ? 'ユーザー名' : 'Username',
          { exact: true }
        );
        const password = page.getByLabel(
          lang === 'ja' ? 'パスワード' : 'Password',
          { exact: true }
        );
        await expect(password).toHaveAttribute('type', 'password');
        await expect(password).toHaveAttribute(
          'autocomplete',
          'current-password'
        );
        await page.evaluate(async () => {
          await document.fonts.ready;
        });
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth
          )
        ).toBe(true);
        const submit = page.getByRole('button', {
          name: lang === 'ja' ? 'ログイン' : 'Sign in',
          exact: true,
        });
        await expect(submit).toHaveCSS('min-height', '44px');
        const colors = await submit.evaluate((el) => ({
          background: getComputedStyle(el).backgroundColor,
          color: getComputedStyle(el).color,
        }));
        expect(colors.background).not.toBe(colors.color);
        await mkdir(directory, { recursive: true });
        await page.screenshot({
          path: `${directory}/${width}-${lang}-${theme}.png`,
        });
        await user.fill('fixture-user');
        await password.fill('fixture-password');
        await submit.click();
        await expect(submit).toBeDisabled();
        await expect(
          page.getByRole('heading', {
            name: '設定UI試作 / Isolated settings prototype',
          })
        ).toBeVisible();
      });
    }
test('failed Basic login shows error and preserves fields with access still blocked', async ({
  page,
}) => {
  await page.goto('/?view=login&auth=basic&lang=en&loginFailure=true');
  await page.getByLabel('Username', { exact: true }).fill('fixture-user');
  await page.getByLabel('Password', { exact: true }).fill('fixture-password');
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page.getByLabel('Username', { exact: true })).toHaveValue(
    'fixture-user'
  );
  await expect(
    page.getByRole('button', { name: 'Sign in', exact: true })
  ).toBeEnabled();
  await expect(
    page.getByRole('heading', {
      name: '設定UI試作 / Isolated settings prototype',
    })
  ).toHaveCount(0);
  await page.screenshot({ path: `${directory}/basic-error.png` });
});
test('Auth0 pending notice and provider handoff stay available', async ({
  page,
}) => {
  await page.goto('/?view=login&auth=auth0&lang=en&access=pending');
  await expect(page.getByText(/Membership approval/)).toBeVisible();
  await expect(
    page.getByText(/Choose a configured sign-in method/)
  ).toBeVisible();
  await page.screenshot({ path: `${directory}/auth0-pending.png` });
  await page
    .getByRole('button', { name: 'Sign in with Auth0', exact: true })
    .click();
  await expect(page).toHaveURL(/fixtureSignedIn=true/);
  await expect(
    page.getByRole('heading', {
      name: '設定UI試作 / Isolated settings prototype',
    })
  ).toBeVisible();
});
test('locked setup and recovery keep their distinct command, secret input and handoff', async ({
  page,
}) => {
  for (const recovery of [false, true]) {
    await page.goto(
      '/?view=login&auth=locked&lang=en' + (recovery ? '&access=recovery' : '')
    );
    await expect(
      page.getByText(
        'docker compose exec lvk server ' +
          (recovery ? 'recovery-token' : 'setup-token'),
        { exact: true }
      )
    ).toBeVisible();
    const key = page.getByLabel(recovery ? 'Recovery key' : 'Setup key', {
      exact: true,
    });
    await expect(key).toHaveAttribute('type', 'password');
    await expect(key).toHaveAttribute('autocomplete', 'off');
    await page.screenshot({
      path: `${directory}/${recovery ? 'recovery' : 'setup'}.png`,
    });
    await key.fill('fixture-key');
    await page
      .getByRole('button', { name: 'Start setup', exact: true })
      .click();
    await expect(page).toHaveURL(/access=settings/);
  }
});
test('loading, status failure and configuration errors remain visible without app content', async ({
  page,
}) => {
  await page.goto('/?view=login&auth=basic&lang=en&statusDelay=1000');
  await expect(
    page.getByText('Checking authentication status…', { exact: true })
  ).toBeVisible();
  await expect(page.getByLabel('Username', { exact: true })).toBeVisible();
  await page.goto('/?view=login&auth=basic&lang=en&statusFailure=true');
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(
    page.getByRole('heading', {
      name: '設定UI試作 / Isolated settings prototype',
    })
  ).toHaveCount(0);
  await page.goto('/?view=login&auth=basic&lang=en&configurationError=true');
  await expect(page.getByText(/Access remains protected/)).toBeVisible();
  await page.screenshot({ path: `${directory}/configuration-error.png` });
});

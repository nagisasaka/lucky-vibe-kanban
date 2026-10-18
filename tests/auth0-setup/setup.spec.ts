import { expect, test } from '@playwright/test';

for (const mobile of [false, true]) {
  test(`Auth0 setup previews, applies and hands off to login (${mobile ? 'mobile dark' : 'desktop light'})`, async ({ page }) => {
    await page.setViewportSize(mobile ? { width: 390, height: 844 } : { width: 1280, height: 900 });
    await page.emulateMedia({ colorScheme: mobile ? 'dark' : 'light' });
    const calls: string[] = [];
    const managementToken = 'm'.repeat(8920);
    let failInspect = true;
    let failApply = true;
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname;
      const respond = (json: unknown, status = 200) => route.fulfill({ status, json });
      if (path === '/api/access/status') return respond({ mode: 'locked', deployment: 'public', management: true, limited: true, authenticated: false, csrf: 'fixture-csrf', public_url: 'https://lvk.example', user: null, forced_mode: null, configuration_error: false });
      if (path === '/api/access/configuration') return respond({ public_url: 'https://lvk.example', callback_url: 'https://lvk.example/api/access/callback', logout_url: 'https://lvk.example/', resource: 'https://lvk.example/mcp', scopes: ['lvk:read','lvk:write','lvk:merge'], current: {} });
      if (path === '/api/access/confirmation') return respond({ ready: false });
      if (path === '/api/access/auth0/provision') {
        const input = route.request().postDataJSON();
        expect(route.request().headers()['x-lvk-csrf']).toBe('fixture-csrf');
        expect(input.management_token).toBe(managementToken);
        calls.push(input.step);
        if (input.step === 'inspect' && failInspect) { failInspect = false; return respond({ error: 'Fixture inspect failure' }, 400); }
        if (input.step === 'inspect') return respond({ clients: [{ client_id: 'existing-web', name: 'Existing Web' }], connections: [{ id: 'con_github', name: 'GitHub', strategy: 'github' }] });
        if (input.step === 'preview') return respond({ confirmation: 'fixture-confirmation', plan: { web: { id: null, settings: { callbacks: ['https://lvk.example/api/access/callback'] } }, chatgpt: { id: null, settings: { callbacks: [input.chatgpt_callback] } }, resource: { id: null, settings: {} }, tenant: { resource_parameter_profile: 'compatibility' } } });
        expect(input.confirmation).toBe('fixture-confirmation');
        if (failApply) { failApply = false; return respond({ error: 'Fixture partial failure; preview again' }, 400); }
        return respond({ issuer: input.issuer, mcp_enabled: true, resource: 'https://lvk.example/mcp', web: { client_id: 'web-created', client_secret: 'fixture-web-secret' }, chatgpt: { client_id: 'chat-created', client_secret: 'fixture-chat-secret' } });
      }
      if (path === '/api/access/auth0/test') {
        const input = route.request().postDataJSON();
        expect(input.client_id).toBe('web-created');
        expect(input.client_secret).toBe('fixture-web-secret');
        expect(input.mcp_enabled).toBe(true);
        expect(input.management_token).toBeUndefined();
        calls.push('login-test');
        return respond({ authorize_url: '/fixture-authorize' });
      }
      return respond({ error: 'Unexpected fixture request' }, 404);
    });
    await page.goto('/?access=settings');
    await page.evaluate((dark) => document.documentElement.classList.toggle('dark', dark), mobile);
    await page.getByText('Auth0を自動セットアップ', { exact: true }).click();
    await page.getByLabel('設定先のAuth0 issuer（末尾 / を含む）').fill('https://fixture.auth0.com/');
    const token = page.getByLabel('Management APIアクセストークン');
    await token.fill(managementToken);
    await page.getByRole('button', { name: '接続とアプリを取得' }).click();
    await expect(page.getByRole('alert')).toHaveText('Fixture inspect failure');
    await expect(token).toHaveValue(managementToken);
    await page.getByRole('button', { name: '接続とアプリを取得' }).click();
    await page.getByLabel('ChatGPT向けMCPも設定する').check();
    await page.getByLabel('ChatGPTに表示された正確なCallback URL').fill('https://chatgpt.com/callback');
    await page.getByRole('button', { name: '変更内容を確認', exact: true }).click();
    expect(calls).toEqual(['inspect','inspect','preview']);
    const apply = page.getByRole('button', { name: 'この内容をAuth0へ適用' });
    await expect(apply).toBeVisible();
    // Editing any plan input invalidates the previous confirmation in the UI.
    await page.getByLabel('ChatGPTに表示された正確なCallback URL').fill('https://chatgpt.com/callback-new');
    await expect(apply).toHaveCount(0);
    await page.getByRole('button', { name: '変更内容を確認', exact: true }).click();
    await apply.click();
    await expect(page.getByRole('alert')).toContainText('Fixture partial failure');
    await expect(page.getByRole('alert')).toContainText('一部の変更が適用されている可能性');
    await expect(token).toHaveValue('');
    await expect(apply).toHaveCount(0);
    await token.fill(managementToken);
    await page.getByRole('button', { name: '変更内容を確認', exact: true }).click();
    await apply.click();
    await expect(token).toHaveValue('');
    await expect(page.getByLabel('Web用Client ID', { exact: true })).toHaveValue('web-created');
    await expect(page.getByLabel('Web用Client Secret', { exact: true })).toHaveValue('fixture-web-secret');
    await expect(page.getByLabel('ChatGPT用Client Secret', { exact: true })).toHaveAttribute('type','password');
    await expect(page.getByLabel('OAuthで保護したリモートHTTP MCPを有効化')).toBeChecked();
    const storage = await page.evaluate(() => JSON.stringify({ local: { ...localStorage }, session: { ...sessionStorage } }));
    expect(storage).not.toContain(managementToken);
    expect(storage).not.toContain('fixture-chat-secret');
    await page.screenshot({ path: `test-results/auth0-setup/${mobile ? 'mobile-dark' : 'desktop-light'}.png`, fullPage: true });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.getByRole('button', { name: '設定をチェックしてログイン試験' }).click();
    await expect(page).toHaveURL(/fixture-authorize/);
    expect(calls.at(-1)).toBe('login-test');
  });
}

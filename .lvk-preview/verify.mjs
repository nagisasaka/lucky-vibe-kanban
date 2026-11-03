const base = 'https://preview2.luckyvibekanban.com';
const html = await (await fetch(base)).text();
if (!html.includes('/@vite/client')) throw Error('Vite client missing');
const client = await (await fetch(base + '/@vite/client')).text();
const token = client.match(/const wsToken = "([^"]+)"/)?.[1];
if (!token) throw Error('HMR token missing');
await new Promise((resolve, reject) => {
  const ws = new WebSocket(base.replace('https:', 'wss:') + '/?token=' + encodeURIComponent(token), 'vite-hmr');
  const timer = setTimeout(() => { ws.close(); reject(Error('HMR timeout')); }, 10000);
  ws.onmessage = ({ data }) => {
    if (JSON.parse(data).type === 'connected') { clearTimeout(timer); ws.close(); resolve(); }
  };
  ws.onerror = () => { clearTimeout(timer); reject(Error('HMR connection failed')); };
});
for (const [route, origin, expected] of [
  ['/api/access/status', base, 200],
  ['/api/access/status', 'https://untrusted.invalid', 403],
  ['/api/local/projects', base, 401],
]) {
  const response = await fetch(base + route, { headers: { Origin: origin } });
  if (response.status !== expected) throw Error(`${route}: ${response.status}`);
}
console.log('Public Vite and HMR connection, API connectivity, origin rejection and authentication boundary passed. Interactive Auth0 login not tested.');

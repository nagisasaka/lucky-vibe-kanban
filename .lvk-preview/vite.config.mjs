import { loadConfigFromFile } from "../packages/local-web/node_modules/vite/dist/node/index.js";
import { fileURLToPath } from "node:url";
import path from "node:path";
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const web = path.join(repo, "packages/local-web");
const loaded = await loadConfigFromFile(
  { command: "serve", mode: "development" },
  path.join(web, "vite.config.ts"),
  web,
);
if (!loaded) throw Error("Existing Vite configuration missing");
export default {
  ...loaded.config,
  root: web,
  envDir: path.join(repo, ".build/ui4-preview/empty-env"),
  cacheDir: path.join(web, "node_modules/.vite-ui4-preview"),
  server: {
    ...loaded.config.server,
    host: "127.0.0.1",
    port: 18444,
    strictPort: true,
    open: false,
    allowedHosts: ["preview2.luckyvibekanban.com"],
    cors: { origin: "https://preview2.luckyvibekanban.com" },
    hmr: {
      protocol: "wss",
      host: "preview2.luckyvibekanban.com",
      clientPort: 443,
    },
    proxy: {
      "/api": {
        target: "http://127.0.0.1:4020",
        changeOrigin: false,
        ws: true,
        headers: { host: "dev.luckyvibekanban.com" },
        configure(proxy) {
          const forwardOrigin = (proxyRequest, request) => {
            if (request.headers.origin === "https://preview2.luckyvibekanban.com") {
              proxyRequest.setHeader("origin", "https://dev.luckyvibekanban.com");
            }
          };
          proxy.on("proxyReq", forwardOrigin);
          proxy.on("proxyReqWs", forwardOrigin);
        },
      },
    },
    fs: {
      strict: true,
      allow: [
        "packages/local-web/index.html",
        "packages/public",
        "packages/local-web/src",
        "packages/web-core/src",
        "packages/ui/src",
        "shared",
        "node_modules",
        "packages/local-web/node_modules",
        "packages/web-core/node_modules",
        "packages/ui/node_modules",
      ].map((p) => path.join(repo, p)),
      deny: [
        "**/.build/**",
        "**/.lvk-preview/**",
        "**/.git/**",
        "**/.env*",
        "**/*.{pem,crt,key,sqlite,sqlite3,db,token}",
        "**/db.v2.sqlite*",
        "**/access-*.json",
        "**/auth*.json",
      ],
    },
  },
};

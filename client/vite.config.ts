import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

export default defineConfig(({ command, mode }) => ({
  plugins: [react()],
  server:
    command === "serve"
      ? {
          proxy: {
            "/api": {
              target: getServerUrl(mode),
              rewrite: (path) => path.replace(/^\/api/, ""),
            },
          },
        }
      : undefined,
}));

const SERVER_DIR = decodeURIComponent(new URL("../server", import.meta.url).pathname);

/** Reads the server's `HOST` and `PORT` the same way it does: env vars win over `server/.env`. */
const getServerUrl = (mode: string): string => {
  const env = loadEnv(mode, SERVER_DIR, ["HOST", "PORT"]);
  const host = env.HOST?.trim();
  const port = env.PORT?.trim();
  if (!host || !port) {
    throw new Error(`HOST and PORT must be set in ${SERVER_DIR}/.env or the environment`);
  }

  // `0.0.0.0` accepts connections on every interface but isn't an address you can connect to.
  const connectableHost = host === "0.0.0.0" ? "127.0.0.1" : host;
  return `http://${connectableHost}:${port}`;
};

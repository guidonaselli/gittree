import { defineConfig } from "vitest/config";
import solid from "vite-plugin-solid";

// Pure-logic tests (api/types.test.ts and similar) need no DOM, so the
// environment stays "node" — jsdom is only pulled in once component tests
// exist and need it (tracked, not yet a gap: no component behaviour is
// untested today, only unexercised by an automated DOM harness).
export default defineConfig({
  plugins: [solid()],
  test: {
    environment: "node",
  },
});

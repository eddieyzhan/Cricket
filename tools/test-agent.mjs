import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
const root =
  process.platform === "win32"
    ? process.env.APPDATA
    : process.platform === "darwin"
      ? join(homedir(), "Library", "Application Support")
      : process.env.XDG_DATA_HOME || join(homedir(), ".local", "share");
const { url, token } = JSON.parse(
  await readFile(
    process.env.CRICKET_AGENT_FILE ||
      join(root, "app.cricket.desktop", "agent.json"),
    "utf8",
  ),
);
assert.equal(new URL(url).hostname, "127.0.0.1");
const query = (path, headers = {}) =>
  fetch(`${url}${path}`, { headers, redirect: "error" });
assert.equal((await query("/v1/state")).status, 401);
assert.equal(
  (await query("/v1/state", { Authorization: "Bearer incorrect" })).status,
  401,
);
assert.equal(
  (
    await query("/v1/state", {
      Authorization: `Bearer ${token}`,
      Origin: "https://example.com",
    })
  ).status,
  401,
);
const response = await query("/v1/state", { Authorization: `Bearer ${token}` });
assert.equal(response.status, 200);
const text = await response.text();
const state = JSON.parse(text);
assert(!text.includes(token));
assert(!text.includes('"code"'));
assert(!text.includes('"access_token"'));
assert.equal(
  state.platform,
  process.platform === "win32"
    ? "windows"
    : process.platform === "darwin"
      ? "macos"
      : process.platform,
);
assert(Array.isArray(state.chats));
assert(Array.isArray(state.transfers));
const invalid = await fetch(`${url}/v1/send`, {
  method: "POST",
  headers: {
    Authorization: `Bearer ${token}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify({ repo: "invalid", paths: [], unrecognized: true }),
});
assert.equal(invalid.status, 422);
console.log(
  "PASS: native app API, loopback binding, authentication, browser Origin rejection, platform detection, secret-free snapshots, unknown-field rejection.",
);

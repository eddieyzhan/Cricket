#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join, isAbsolute } from "node:path";

const help = `Cricket agent interface (the desktop app must be running)

  node tools/cricket.mjs status
  node tools/cricket.mjs sync
  node tools/cricket.mjs create-chat --name "My devices" [--member github-login]
  node tools/cricket.mjs import-chat --chat owner/cricket-id
  node tools/cricket.mjs send --chat owner/cricket-id --file /absolute/file [--file /another]
  node tools/cricket.mjs receive --transfer owner/cricket-id#1 --directory /absolute/downloads
  node tools/cricket.mjs retry --transfer owner/cricket-id#1 --recipient github-login
  node tools/cricket.mjs request-retry --transfer owner/cricket-id#1
  node tools/cricket.mjs cancel --job job-id

All responses are JSON. Read docs/AGENT.md before sending files.
CRICKET_AGENT_FILE can override the local discovery file location.
`;
const args = process.argv.slice(2);
const command = args.shift();
if (!command || ["help", "--help", "-h"].includes(command)) {
  console.log(help);
  process.exit(0);
}
try {
  const options = new Map();
  while (args.length) {
    const key = args.shift();
    const value = args.shift();
    if (!key.startsWith("--") || !value || value.startsWith("--"))
      throw new Error(`Expected a value for ${key}`);
    options.set(key, [...(options.get(key) ?? []), value]);
  }
  const required = (key) => {
    const values = options.get(key);
    if (values?.length !== 1) throw new Error(`Supply ${key} exactly once.`);
    return values[0];
  };
  const absolute = (value) => {
    if (!isAbsolute(value)) throw new Error("Use absolute filesystem paths.");
    return value;
  };
  let endpoint;
  let body;
  const allowed = {
    status: [],
    sync: [],
    "create-chat": ["--name", "--member"],
    "import-chat": ["--chat"],
    send: ["--chat", "--file"],
    receive: ["--transfer", "--directory"],
    retry: ["--transfer", "--recipient"],
    "request-retry": ["--transfer"],
    cancel: ["--job"],
  }[command];
  if (!allowed) throw new Error(`Unknown command: ${command}`);
  for (const key of options.keys())
    if (!allowed.includes(key)) throw new Error(`Unknown option: ${key}`);
  switch (command) {
    case "create-chat":
      endpoint = "chats";
      body = {
        name: required("--name"),
        members: options.get("--member") ?? [],
      };
      break;
    case "import-chat":
      endpoint = "chats/import";
      body = { repo: required("--chat") };
      break;
    case "status":
      endpoint = "state";
      break;
    case "sync":
      endpoint = "sync";
      body = {};
      break;
    case "send": {
      const paths = (options.get("--file") ?? []).map(absolute);
      if (!paths.length) throw new Error("Supply at least one --file.");
      endpoint = "send";
      body = { repo: required("--chat"), paths };
      break;
    }
    case "receive":
      endpoint = "receive";
      body = {
        key: required("--transfer"),
        directory: absolute(required("--directory")),
      };
      break;
    case "retry":
      endpoint = "retry";
      body = {
        key: required("--transfer"),
        recipient: required("--recipient"),
      };
      break;
    case "request-retry":
      endpoint = "request-retry";
      body = { key: required("--transfer") };
      break;
    case "cancel":
      endpoint = "cancel";
      body = { job_id: required("--job") };
      break;
  }
  const root =
    process.platform === "win32"
      ? process.env.APPDATA
      : process.platform === "darwin"
        ? join(homedir(), "Library", "Application Support")
        : process.env.XDG_DATA_HOME || join(homedir(), ".local", "share");
  const file =
    process.env.CRICKET_AGENT_FILE ||
    join(root, "app.cricket.desktop", "agent.json");
  const discovery = JSON.parse(await readFile(file, "utf8"));
  const url = new URL(discovery.url);
  if (
    url.protocol !== "http:" ||
    url.hostname !== "127.0.0.1" ||
    !url.port ||
    url.username ||
    url.password ||
    url.pathname !== "/"
  )
    throw new Error("Invalid loopback discovery file.");
  const response = await fetch(`${url.origin}/v1/${endpoint}`, {
    method: body ? "POST" : "GET",
    redirect: "error",
    headers: {
      Authorization: `Bearer ${discovery.token}`,
      "Content-Type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
    signal: AbortSignal.timeout(120_000),
  });
  const result = await response.json();
  console.log(JSON.stringify(result, null, 2));
  if (!response.ok) process.exitCode = 1;
} catch (error) {
  console.error(
    JSON.stringify({
      error:
        error.code === "ENOENT"
          ? "Open Cricket first. Its local agent interface is not running."
          : error.message,
    }),
  );
  process.exitCode = 1;
}

import { spawnSync } from "node:child_process";
import { homedir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const [command, ...args] = process.argv.slice(2);
if (!["cargo", "tauri"].includes(command))
  throw new Error("Use native.mjs cargo|tauri <arguments>");
const env = { ...process.env };
if (command === "cargo" || args[0] === "build") {
  if (env.RUSTFLAGS && !env.CARGO_ENCODED_RUSTFLAGS)
    throw new Error(
      "Use CARGO_ENCODED_RUSTFLAGS for custom flags with privacy-preserving builds.",
    );
  const mappings = [
    ...new Set([homedir(), homedir().replaceAll("\\", "/")]),
  ].map((p) => `${p}=/build`);
  mappings.push(
    ...[...new Set([root, root.replaceAll("\\", "/")])].map(
      (p) => `${p}=/src/cricket`,
    ),
  );
  env.CARGO_ENCODED_RUSTFLAGS = [
    env.CARGO_ENCODED_RUSTFLAGS,
    ...mappings.map((p) => `--remap-path-prefix=${p}`),
  ]
    .filter(Boolean)
    .join("\x1f");
  delete env.RUSTFLAGS;
}
const result =
  command === "tauri"
    ? spawnSync(
        process.execPath,
        [resolve(root, "node_modules/@tauri-apps/cli/tauri.js"), ...args],
        { env, stdio: "inherit", windowsHide: true },
      )
    : spawnSync("cargo", args, { env, stdio: "inherit", windowsHide: true });
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;

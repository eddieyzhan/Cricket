// Run only on a fresh CI Mac. Test the app copied from the actual DMG, not the checkout.
import assert from "node:assert/strict";
import { execFileSync, spawn } from "node:child_process";
import {
  access,
  cp,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
assert.equal(process.platform, "darwin");
assert(
  process.env.CI || process.env.CM_BUILD_ID,
  "Only run on a fresh CI machine",
);
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const version = JSON.parse(
  await readFile(join(root, "package.json"), "utf8"),
).version;
const data = join(homedir(), "Library/Application Support/app.cricket.desktop");
await assert.rejects(
  access(data),
  "Refusing to use an existing Cricket profile",
);
const temp = await mkdtemp(join(tmpdir(), "cricket-package-"));
const mount = join(temp, "mounted");
const dmgs = join(root, "src-tauri/target/release/bundle/dmg");
const files = (await readdir(dmgs)).filter(
  (f) => f.startsWith(`Cricket_${version}_`) && f.endsWith(".dmg"),
);
assert.equal(files.length, 1);
let mounted = false;
let app;
try {
  await mkdir(mount);
  execFileSync(
    "hdiutil",
    [
      "attach",
      "-nobrowse",
      "-readonly",
      "-mountpoint",
      mount,
      join(dmgs, files[0]),
    ],
    { stdio: "ignore" },
  );
  mounted = true;
  const relocated = join(temp, "Cricket.app");
  await cp(join(mount, "Cricket.app"), relocated, {
    recursive: true,
    verbatimSymlinks: true,
  });
  const croc = join(relocated, "Contents/Resources/binaries/croc");
  assert.match(
    execFileSync(croc, ["--version"], { encoding: "utf8" }),
    /11\.5\.3/,
  );
  app = spawn(join(relocated, "Contents/MacOS/cricket"), [], {
    cwd: temp,
    stdio: "ignore",
  });
  app.on("error", () => {});
  const agentFile = join(data, "agent.json");
  let ready = false;
  for (let attempt = 0; attempt < 60; attempt++) {
    if (app.exitCode !== null)
      throw new Error("Packaged app exited before startup");
    try {
      const agent = JSON.parse(await readFile(agentFile, "utf8"));
      assert.equal(agent.pid, app.pid);
      const state = await fetch(`${agent.url}/v1/state`, {
        headers: { Authorization: `Bearer ${agent.token}` },
        signal: AbortSignal.timeout(2000),
      }).then((r) => r.json());
      if (state.croc_version?.includes("11.5.3")) {
        ready = true;
        break;
      }
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  assert(ready, "Packaged app did not discover its bundled croc");
  execFileSync(process.execPath, [join(root, "tools/test-agent.mjs")], {
    env: { ...process.env, CRICKET_AGENT_FILE: agentFile },
    stdio: "inherit",
    timeout: 15000,
  });
  await mkdir(join(root, "artifacts"), { recursive: true });
  await writeFile(
    join(root, "artifacts/macos-validation.json"),
    JSON.stringify(
      {
        version,
        platform: process.platform,
        arch: process.arch,
        dmg: files[0],
        relocatedStartup: true,
        bundledCroc: "11.5.3",
        authenticatedAgentChecks: true,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(
    "PASS: relocated DMG app startup, bundled croc, native authenticated API.",
  );
} finally {
  if (app && app.exitCode === null) {
    app.kill();
    await Promise.race([
      new Promise((r) => app.once("exit", r)),
      new Promise((r) => setTimeout(r, 3000)),
    ]);
    if (app.exitCode === null) app.kill("SIGKILL");
  }
  if (mounted) execFileSync("hdiutil", ["detach", mount], { stdio: "ignore" });
  await rm(temp, { recursive: true, force: true });
}

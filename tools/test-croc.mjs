// A real round trip through Cricket's pinned croc binary. Auto transport by default;
// --public-relay opts into sending a generated test fixture via croc's public relay.
// --force-relay tests relay fallback explicitly. Never used by the application.
import { spawn } from "node:child_process";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  writeFile,
  rm,
  stat,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import net from "node:net";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const binary = join(
  root,
  "src-tauri",
  "binaries",
  process.platform === "win32" ? "croc.exe" : "croc",
);
const temp = await mkdtemp(join(tmpdir(), "cricket-test-"));
const children = [];
let secret = "";
const logs = [];
function launch(args, env = {}) {
  const child = spawn(binary, args, {
    env: { ...process.env, ...env },
    stdio: [
      "ignore",
      process.argv.includes("--null-stdout") ? "ignore" : "pipe",
      "pipe",
    ],
    windowsHide: true,
  });
  children.push(child);
  child.stdout?.on("data", (b) => logs.push(b.toString()));
  child.stderr.on("data", (b) => logs.push(b.toString()));
  child.finished = new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (code) =>
      code === 0 ? resolve() : reject(new Error(`croc exited with ${code}`)),
    );
  });
  child.finished.catch(() => {});
  return child;
}
async function waitForPort(port) {
  for (let i = 0; i < 80; i++) {
    if (
      await new Promise((resolve) => {
        const s = net.connect(port, "127.0.0.1");
        s.on("connect", () => {
          s.destroy();
          resolve(true);
        });
        s.on("error", () => resolve(false));
      })
    )
      return;
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error("Local relay did not start");
}
try {
  const port = 45000 + Math.floor(Math.random() * 10000);
  const publicRelay = process.argv.includes("--public-relay");
  if (!publicRelay)
    launch([
      "relay",
      "--host",
      "127.0.0.1",
      "--ports",
      `${port},${port + 1},${port + 2},${port + 3},${port + 4}`,
    ]);
  if (!publicRelay) await waitForPort(port);
  const input = join(temp, "a file with spaces.txt");
  const output = join(temp, "received");
  await mkdir(output);
  await writeFile(input, "Cricket round-trip verification. 🦗\n".repeat(2500));
  const folderTest = process.argv.includes("--folder");
  const sendFolder = join(temp, "folder with spaces");
  if (folderTest) {
    await mkdir(join(sendFolder, "nested"), { recursive: true });
    await mkdir(join(sendFolder, "empty"));
    await writeFile(
      join(sendFolder, "nested", "unicode-🦗.txt"),
      await readFile(input),
    );
    await writeFile(
      join(sendFolder, "random.bin"),
      randomBytes(2 * 1024 * 1024),
    );
  }
  const relayIndexArg = process.argv.indexOf("--relay-index");
  const relayIndex =
    relayIndexArg < 0 ? null : Number(process.argv[relayIndexArg + 1]);
  if (
    relayIndex !== null &&
    (!Number.isInteger(relayIndex) || relayIndex < 0 || relayIndex > 3)
  )
    throw new Error("--relay-index must be 0, 1, 2, or 3");
  do {
    secret = `${randomUUID()}-${randomUUID()}`;
  } while (
    relayIndex !== null &&
    createHash("sha256").update(secret).digest()[31] % 4 !== relayIndex
  );
  if (publicRelay)
    console.log(
      `Testing public relay ${(createHash("sha256").update(secret).digest()[31] % 4) + 1}.`,
    );
  const env = {
    CROC_SECRET: secret,
    ...(publicRelay
      ? {}
      : { CROC_RELAY: `127.0.0.1:${port}`, CROC_RELAY6: `127.0.0.1:${port}` }),
  };
  const flags = ["--yes", "--disable-clipboard", "--ignore-stdin"];
  const sender = launch(
    [
      ...flags,
      "send",
      ...(process.argv.includes("--force-relay")
        ? ["--transport", "relay", "--no-local"]
        : []),
      "--",
      folderTest ? sendFolder : input,
    ],
    env,
  );
  await new Promise((r) => setTimeout(r, 1000));
  const receiver = launch([...flags, "--out", output], env);
  let timer;
  try {
    await Promise.race([
      Promise.all([sender.finished, receiver.finished]),
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error("Round trip timed out")),
          45000,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
  const hash = async (file) =>
    createHash("sha256")
      .update(await readFile(file))
      .digest("hex");
  if (folderTest) {
    for (const file of [join("nested", "unicode-🦗.txt"), "random.bin"]) {
      if (
        (await hash(join(sendFolder, file))) !==
        (await hash(join(output, "folder with spaces", file)))
      )
        throw new Error("Received folder content does not match");
    }
    if (
      !(await stat(join(output, "folder with spaces", "empty"))).isDirectory()
    )
      throw new Error("Empty folder was not received");
  } else if (
    (await hash(input)) !== (await hash(join(output, "a file with spaces.txt")))
  ) {
    throw new Error("Received file content does not match");
  }
  console.log(
    `PASS: real croc send + receive, ${folderTest ? "nested folder, empty folder, Unicode filename, binary content" : "spaced filename, UTF-8 content"}, SHA-256 equality, ${process.argv.includes("--force-relay") ? "explicit relay fallback" : "normal automatic transport"}, ${publicRelay ? "public rendezvous" : "loopback rendezvous"}.`,
  );
} catch (error) {
  console.error(error.message);
  console.error(logs.join("").split(secret).join("[redacted]").slice(-5000));
  process.exitCode = 1;
} finally {
  for (const child of children) if (child.exitCode === null) child.kill();
  await new Promise((r) => setTimeout(r, 200));
  await rm(temp, { recursive: true, force: true });
}

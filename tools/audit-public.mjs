// Audit reachable Git history and staged/working project files without printing secrets.
// Optional file arguments additionally inspect uncompressed release executables.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { homedir, hostname, platform } from "node:os";
import { join } from "node:path";
const git = (...args) =>
  execFileSync("git", args, {
    encoding: "utf8",
    windowsHide: true,
    maxBuffer: 32 * 1024 * 1024,
  });
const groups = new Map([
  [
    "local user path",
    [
      homedir(),
      homedir().replaceAll("\\", "/"),
      homedir().replaceAll("\\", "\\\\"),
    ],
  ],
  ["device name", [hostname()]],
]);
const dataDir =
  platform() === "win32"
    ? join(process.env.APPDATA, "app.cricket.desktop")
    : platform() === "darwin"
      ? join(homedir(), "Library/Application Support/app.cricket.desktop")
      : join(
          process.env.XDG_DATA_HOME || join(homedir(), ".local/share"),
          "app.cricket.desktop",
        );
try {
  const s = JSON.parse(readFileSync(join(dataDir, "state.json"), "utf8"));
  groups.set("device identifier", [s.device_id, s.device_name]);
  groups.set(
    "private chat identifier",
    (s.chats || []).map((c) => c.repo),
  );
  groups.set("saved contact", s.contacts || []);
  groups.set(
    "transfer secret",
    (s.transfers || []).flatMap((t) => [
      ...(t.offer.slots || []).map((x) => x.code),
      ...(t.events || []).map((x) => x.event.code),
    ]),
  );
  const d = JSON.parse(readFileSync(join(dataDir, "agent.json"), "utf8"));
  groups.set("agent credential", [d.token]);
} catch (e) {
  if (e.code !== "ENOENT") throw e;
}
const needles = [...groups].map(([category, values]) => [
  category,
  values
    .filter((v) => typeof v === "string" && v.length >= 4)
    .flatMap((v) => [Buffer.from(v), Buffer.from(v, "utf16le")]),
]);
const findings = new Set();
let inspected = 0;
function inspect(data, name) {
  inspected++;
  for (const [category, values] of needles)
    if (values.some((v) => data.includes(v)))
      findings.add(`${name}: ${category}`);
  if (
    /(?:gh[pousr]_[A-Za-z0-9_]{30,}|github_pat_[A-Za-z0-9_]{30,}|AKIA[A-Z0-9]{16}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----)/.test(
      data.toString("utf8"),
    )
  )
    findings.add(`${name}: credential pattern`);
}
for (const row of git("rev-list", "--objects", "--all").trim().split("\n")) {
  const [oid, ...parts] = row.split(" ");
  const name = parts.join(" ") || "commit metadata";
  if (!["blob", "commit", "tag"].includes(git("cat-file", "-t", oid).trim()))
    continue;
  inspect(
    execFileSync("git", ["cat-file", "-p", oid], {
      windowsHide: true,
      maxBuffer: 32 * 1024 * 1024,
    }),
    name,
  );
}
const projectFiles = git(
  "ls-files",
  "--cached",
  "--others",
  "--exclude-standard",
  "-z",
)
  .split("\0")
  .filter(Boolean);
for (const path of projectFiles) {
  if (
    /(^|\/)(\.local|artifacts|node_modules|target)(\/|$)|(^|\/)(state|agent|contacts)\.json$|(^|\/)\.env(?:\.|$)/.test(
      path,
    )
  )
    findings.add(`${path}: local data tracked or unignored`);
  try {
    inspect(readFileSync(path), path);
  } catch (e) {
    if (e.code !== "ENOENT") throw e;
  }
}
for (const path of process.argv.slice(2)) inspect(readFileSync(path), path);
console.log(`Inspected ${inspected} Git objects/project files/binaries.`);
if (findings.size) {
  console.error([...findings].join("\n"));
  process.exitCode = 1;
} else
  console.log(
    "PASS: no detected private runtime data, local account paths, or credentials. Public repository identity and fictional fixtures remain intentional.",
  );

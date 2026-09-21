import { createHash } from "node:crypto";
import {
  mkdtemp,
  mkdir,
  writeFile,
  copyFile,
  chmod,
  rm,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const version = "11.5.3";
const builds = {
  "win32-x64": [
    "Windows-64bit.zip",
    "b9163fb162523f3acaabf33de38797545b557bf470808c97758aa9f9ca16b859",
  ],
  "win32-arm64": [
    "Windows-ARM64.zip",
    "523e634bd8d05020e019f8645c94cf02c146c85dbbee7d90cd3724931af3e07f",
  ],
  "darwin-x64": [
    "macOS-64bit.tar.gz",
    "438b87becaf3cd7b11e6316f1bf8cf0b6478d14b13b0e19bada6c2f943393a50",
  ],
  "darwin-arm64": [
    "macOS-ARM64.tar.gz",
    "cfa99d0f669ab604a19e7ea7ddbd8993adb9eeff1ab8524c902c9c6f65491dff",
  ],
  "linux-x64": [
    "Linux-64bit.tar.gz",
    "73b44449937a3f656571c4f016cb2fd0e722544568d7a17dcc3b82475ad250ad",
  ],
  "linux-arm64": [
    "Linux-ARM64.tar.gz",
    "fd3520cbb0062dcccb60f6f66fd6d5369d4655b567b772f4e5eef4a0fd794ce8",
  ],
};
const build = builds[`${process.platform}-${process.arch}`];
if (!build)
  throw new Error(
    `Unsupported build host: ${process.platform}-${process.arch}`,
  );
const [asset, checksum] = build;
const folder = await mkdtemp(join(tmpdir(), "cricket-croc-"));
try {
  const url = `https://github.com/schollz/croc/releases/download/v${version}/croc_v${version}_${asset}`;
  console.log(
    `Downloading pinned croc ${version} for ${process.platform}/${process.arch}…`,
  );
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Download failed: ${response.status}`);
  const data = Buffer.from(await response.arrayBuffer());
  if (createHash("sha256").update(data).digest("hex") !== checksum)
    throw new Error("croc checksum mismatch. Nothing was installed.");
  const archive = join(folder, asset);
  await writeFile(archive, data);
  const binary = process.platform === "win32" ? "croc.exe" : "croc";
  execFileSync("tar", ["-xf", archive, "-C", folder, binary, "LICENSE"], {
    windowsHide: true,
  });
  const target = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../src-tauri/binaries",
  );
  await mkdir(target, { recursive: true });
  await copyFile(join(folder, binary), join(target, binary));
  await copyFile(join(folder, "LICENSE"), join(target, "croc-LICENSE.txt"));
  if (process.platform !== "win32") await chmod(join(target, binary), 0o755);
  console.log(
    `Verified SHA-256. croc is ready in src-tauri/binaries/${binary}.`,
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}

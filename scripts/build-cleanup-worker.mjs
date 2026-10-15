#!/usr/bin/env node
// Builds wordfall-cleanup, the helper that runs built-in cleanup models, and
// puts it where the installer build expects it:
// src-tauri/binaries/wordfall-cleanup-<target>[.exe].
//
//   npm run worker:build                                    this computer
//   npm run worker:build -- --target universal-apple-darwin Apple Silicon and Intel in one
//
// Then build installers with the helper included:
//
//   npx tauri build --config src-tauri/tauri.cleanup.conf.json [--target …]
//
// `npm run app:build` does both for this computer. While developing, `npm run app`
// finds the helper in cleanup-worker/target, so `cargo build --release` there is enough.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const desktop = join(dirname(fileURLToPath(import.meta.url)), "..");
const worker = join(desktop, "cleanup-worker");
const binaries = join(desktop, "src-tauri", "binaries");
const NAME = "wordfall-cleanup";

const run = (command, args, cwd = desktop) => execFileSync(command, args, { cwd, stdio: "inherit" });

const args = process.argv.slice(2);
const targetIndex = args.indexOf("--target");
const hostTarget = () =>
  execFileSync("rustc", ["-vV"], { encoding: "utf8" })
    .split("\n")
    .find((line) => line.startsWith("host: "))
    .slice("host: ".length)
    .trim();
const target = targetIndex >= 0 ? args[targetIndex + 1] : hostTarget();
if (!target) {
  console.error("build-cleanup-worker: --target needs a value");
  process.exit(1);
}

/** Builds for one Rust target and copies the binary into src-tauri/binaries. */
function build(triple) {
  run("cargo", ["build", "--release", "--locked", "--target", triple], worker);
  const exe = triple.includes("windows") ? ".exe" : "";
  const dest = join(binaries, `${NAME}-${triple}${exe}`);
  copyFileSync(join(worker, "target", triple, "release", `${NAME}${exe}`), dest);
  console.log(`build-cleanup-worker: ${dest}`);
  return dest;
}

mkdirSync(binaries, { recursive: true });
if (target === "universal-apple-darwin") {
  // A universal app is compiled once per architecture, and each pass looks for
  // its own helper; the bundle then takes the universal one.
  const parts = ["aarch64-apple-darwin", "x86_64-apple-darwin"].map(build);
  const universal = join(binaries, `${NAME}-universal-apple-darwin`);
  run("lipo", ["-create", "-output", universal, ...parts]);
  console.log(`build-cleanup-worker: ${universal}`);
} else {
  build(target);
}

#!/usr/bin/env node
// Writes the release's latest.json, which tells the in-app updater where each
// system's signed build is, and attaches it to the release. CI runs this once
// every installer is uploaded, so the app only offers a release that's ready.
//
//   node scripts/updater-manifest.mjs v2.0.0-alpha.9

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const tag = process.argv[2];
if (!tag?.startsWith("v")) {
  console.error("usage: node scripts/updater-manifest.mjs <tag>");
  process.exit(1);
}

// The updater looks for "{os}-{arch}-{installer}", then "{os}-{arch}".
// The macOS app is universal, so it serves both architectures.
const PLATFORMS = [
  [".app.tar.gz", ["darwin-aarch64", "darwin-aarch64-app", "darwin-x86_64", "darwin-x86_64-app"]],
  ["-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]],
  [".AppImage", ["linux-x86_64", "linux-x86_64-appimage"]],
  [".deb", ["linux-x86_64-deb"]],
  [".rpm", ["linux-x86_64-rpm"]],
];

const gh = (...args) => execFileSync("gh", args, { encoding: "utf8" });

const { assets } = JSON.parse(gh("release", "view", tag, "--json", "assets"));
const dir = mkdtempSync(join(tmpdir(), "wordfall-manifest-"));
gh("release", "download", tag, "--pattern", "*.sig", "--dir", dir);

const platforms = {};
for (const asset of assets) {
  const match = PLATFORMS.find(([suffix]) => asset.name.endsWith(suffix));
  const signed = assets.some((a) => a.name === `${asset.name}.sig`);
  if (!match || !signed) continue;
  const signature = readFileSync(join(dir, `${asset.name}.sig`), "utf8").trim();
  for (const platform of match[1]) platforms[platform] = { url: asset.url, signature };
}

const missing = ["darwin-aarch64", "windows-x86_64", "linux-x86_64"].filter((p) => !platforms[p]);
if (missing.length > 0) {
  console.error(`No signed build for ${missing.join(", ")} on ${tag}`);
  process.exit(1);
}

const manifest = join(dir, "latest.json");
writeFileSync(
  manifest,
  JSON.stringify({ version: tag.slice(1), pub_date: new Date().toISOString(), platforms }, null, 2),
);
gh("release", "upload", tag, manifest, "--clobber");
console.log(`Attached latest.json to ${tag} for ${Object.keys(platforms).join(", ")}`);

/**
 * This script is used to rename the binary with the platform specific postfix.
 * When `tauri build` is ran, it looks for the binary name appended with the platform specific postfix.
 */

import { execa } from "execa";
import { existsSync, renameSync } from "fs";

let extension = "";
if (process.platform === "win32") {
  extension = ".exe";
}

async function main() {
  // TODO - i get a SIGABRT on linux calling this (but i can run it myself perfectly fine...)
  const rustInfo = (await execa("rustc", ["-vV"])).stdout;
  const targetTriple = /host: (\S+)/g.exec(rustInfo)[1];
  if (!targetTriple) {
    console.error("Failed to determine platform target triple");
  }
  if (existsSync(`src-tauri/bin/glewinfo${extension}`)) {
    renameSync(
      `src-tauri/bin/glewinfo${extension}`,
      `src-tauri/bin/glewinfo-${targetTriple}${extension}`
    );
  }
  // TODO else move the binary from third-party into the right spot
}

main().catch((e) => {
  throw e;
});
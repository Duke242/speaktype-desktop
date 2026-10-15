import { describe, expect, it } from "vitest";
import { describeLibraryImport, describeSync, folderName } from "./sharing";

describe("describeSync", () => {
  const now = 1_000_000_000;

  it("says when it last synced", () => {
    expect(describeSync({ lastSyncedAt: now - 5_000, error: null }, now)).toBe("Synced just now.");
    expect(describeSync({ lastSyncedAt: now - 5 * 60_000, error: null }, now)).toBe("Synced 5m ago.");
  });

  it("shows an error over the last sync", () => {
    expect(describeSync({ lastSyncedAt: now, error: "Couldn't find the shared folder." }, now)).toBe(
      "Couldn't find the shared folder.",
    );
  });

  it("covers a folder that hasn't synced yet", () => {
    expect(describeSync(null, now)).toBe("Not synced yet.");
    expect(describeSync({ lastSyncedAt: null, error: null }, now)).toBe("Not synced yet.");
  });
});

describe("describeLibraryImport", () => {
  it("counts what was added", () => {
    expect(describeLibraryImport({ rules: 2, snippets: 1 })).toBe("Imported 2 rules and 1 snippet.");
    expect(describeLibraryImport({ rules: 0, snippets: 3 })).toBe("Imported 3 snippets.");
    expect(describeLibraryImport({ rules: 0, snippets: 0 })).toBe("Everything in that file is already here.");
  });
});

describe("folderName", () => {
  it("takes the last folder on any system", () => {
    expect(folderName("/Users/me/Dropbox/Wordflow")).toBe("Wordflow");
    expect(folderName("C:\\Users\\me\\OneDrive\\Team\\")).toBe("Team");
    expect(folderName("/")).toBe("/");
  });
});

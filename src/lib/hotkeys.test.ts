import { describe, expect, it } from "vitest";
import { acceleratorFrom, heldModifiers, type KeyPress } from "./hotkeys";

const press = (code: string, mods: Partial<KeyPress> = {}): KeyPress => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("acceleratorFrom", () => {
  it("builds accelerators in a fixed modifier order", () => {
    expect(acceleratorFrom(press("Space", { shiftKey: true, ctrlKey: true }))).toBe("Ctrl+Shift+Space");
    expect(acceleratorFrom(press("KeyD", { metaKey: true, altKey: true }))).toBe("Alt+Super+D");
    expect(acceleratorFrom(press("Digit1", { ctrlKey: true }))).toBe("Ctrl+1");
  });

  it("waits while only modifiers are held", () => {
    expect(acceleratorFrom(press("ShiftLeft", { shiftKey: true }))).toBeNull();
    expect(acceleratorFrom(press("MetaRight", { metaKey: true }))).toBeNull();
  });

  it("allows bare function keys but not bare letters", () => {
    expect(acceleratorFrom(press("F5"))).toBe("F5");
    expect(acceleratorFrom(press("KeyA"))).toBeNull();
    expect(acceleratorFrom(press("Space"))).toBeNull();
  });
});

describe("heldModifiers", () => {
  it("lists held modifiers", () => {
    expect(heldModifiers(press("KeyA", { altKey: true, shiftKey: true }))).toBe("Alt+Shift");
    expect(heldModifiers(press("KeyA"))).toBe("");
  });
});

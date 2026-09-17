/** The subset of a KeyboardEvent the hotkey parser reads, so it's testable without a DOM. */
export interface KeyPress {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/** "Ctrl+Alt" for the modifiers held right now, in a fixed order. */
export function heldModifiers(event: KeyPress): string {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Super");
  return parts.join("+");
}

/**
 * Builds an accelerator such as "Ctrl+Shift+Space" from a key press, or null
 * while only modifiers are down. KeyboardEvent.code names ("KeyD", "Digit1",
 * "F5") are what the Rust side parses.
 */
export function acceleratorFrom(event: KeyPress): string | null {
  if (MODIFIER_CODES.has(event.code)) return null;
  const modifiers = heldModifiers(event);
  const key = event.code.replace(/^Key/, "").replace(/^Digit/, "");
  // A bare letter would fire while typing, so require a modifier unless it's a function key.
  if (!modifiers && !/^F\d+$/.test(key)) return null;
  return modifiers ? `${modifiers}+${key}` : key;
}

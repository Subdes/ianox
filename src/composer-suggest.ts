/** What the user is typing at the caret in the message box: a `/command` or an `@file`. */
export type Trigger = { kind: "slash" | "mention"; query: string; start: number; end: number } | null;

export function detectTrigger(text: string, caret: number): Trigger {
  const before = text.slice(0, caret);

  // A command only counts at the very start of the message: "/title", "  /new".
  const slash = /^(\s*)\/([\w-]*)$/.exec(before);
  if (slash) return { kind: "slash", query: slash[2], start: slash[1].length, end: caret };

  // A mention starts after whitespace or at the start: "look at @src/ma".
  const mention = /(^|\s)@([^\s@]*)$/.exec(before);
  if (mention) {
    return { kind: "mention", query: mention[2], start: mention.index + mention[1].length, end: caret };
  }
  return null;
}

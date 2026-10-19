/** The string literals of `pub const NAME: [&str; N] = [...]` (the literal arrays are the single list). */
export function constantNames(src: string, name: string): string[] {
  const m = new RegExp(`pub const ${name}: \\[&str; \\d+\\] = \\[([^\\]]*)\\]`).exec(src);
  return m === null ? [] : [...m[1].matchAll(/"([a-z_]+)"/g)].map((x) => x[1]);
}

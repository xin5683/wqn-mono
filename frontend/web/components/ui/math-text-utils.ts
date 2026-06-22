/** Check whether a string contains `$...$` inline math delimiters. */
export function containsMath(text: string): boolean {
  return /\$[^$]+\$/.test(text);
}

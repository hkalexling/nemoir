import type { Problem } from "../types";

const validPalindrome: Problem = {
  id: "valid-palindrome",
  title: "Valid Palindrome",
  difficulty: "beginner",
  topics: ["two-pointers"],
  statement: `Given a string \`s\`, return \`true\` if it is a palindrome after converting all uppercase letters to lowercase and removing all non-alphanumeric characters.

A palindrome reads the same forward and backward.

Alphanumeric characters include letters and numbers.`,
  constraints: [
    "1 <= s.length <= 2 * 10^5",
    "s consists only of printable ASCII characters.",
  ],
  examples: [
    {
      input: 's = "A man, a plan, a canal: Panama"',
      output: "true",
      explanation: 'After filtering: "amanaplanacanalpanama" reads the same forward and backward.',
    },
    {
      input: 's = "race a car"',
      output: "false",
      explanation: 'After filtering: "raceacar" is not a palindrome.',
    },
    {
      input: 's = " "',
      output: "true",
      explanation: "After removing non-alphanumeric characters the string is empty, and an empty string is considered a palindrome.",
    },
  ],
  entryFunctionName: "isPalindrome",
  starterCode: `function isPalindrome(s) {
  // TODO: return true if the filtered lowercase string is a palindrome
  return false;
}
`,
  visibleTests: [
    { id: "phrase", args: ["A man, a plan, a canal: Panama"], expected: true },
    { id: "not_palindrome", args: ["race a car"], expected: false },
    { id: "empty_after_filter", args: [" "], expected: true },
    { id: "alphanumeric_only", args: ["abBA"], expected: true },
    { id: "single_char", args: ["Z"], expected: true },
    { id: "numbers", args: ["12321"], expected: true },
    { id: "mixed", args: ["1a2"], expected: false },
  ],
  evaluatorVersion: "1.0.0",
  executionLimits: {
    timeoutMs: 3000,
    maxSourceBytes: 65536,
    maxInputBytes: 262144,
    maxOutputBytes: 262144,
  },
};

export default validPalindrome;

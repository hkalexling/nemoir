import type { Problem } from "../types";

const validParentheses: Problem = {
  id: "valid-parentheses",
  title: "Valid Parentheses",
  difficulty: "beginner",
  topics: ["stack"],
  statement: `Given a string \`s\` containing only the characters \`(\`, \`)\`, \`{\`, \`}\`, \`[\`, and \`]\`, determine if the input string is valid.

A string is valid when:
1. Open brackets are closed by the same type of brackets.
2. Open brackets are closed in the correct order.
3. Every closing bracket has a corresponding opening bracket of the same type.`,
  constraints: [
    "1 <= s.length <= 10^4",
    "s consists of parentheses only: ()[]{}",
  ],
  examples: [
    {
      input: 's = "()"',
      output: "true",
      explanation: "A single matching pair.",
    },
    {
      input: 's = "()[]{}"',
      output: "true",
      explanation: "Multiple matching pairs in order.",
    },
    {
      input: 's = "(]"',
      output: "false",
      explanation: "Mismatched bracket types.",
    },
    {
      input: 's = "([])"',
      output: "true",
      explanation: "Nested brackets closed in correct order.",
    },
  ],
  entryFunctionName: "isValidParentheses",
  starterCode: `function isValidParentheses(s) {
  // TODO: return true if the parentheses string is valid
  return false;
}
`,
  visibleTests: [
    { id: "single_pair", args: ["()"], expected: true },
    { id: "multiple_pairs", args: ["()[]{}"], expected: true },
    { id: "mismatched", args: ["(]"], expected: false },
    { id: "nested", args: ["([])"], expected: true },
    { id: "unclosed", args: ["(("], expected: false },
    { id: "single_open", args: ["["], expected: false },
    { id: "empty_like_input", args: [""], expected: true },
  ],
  evaluatorVersion: "1.0.0",
  executionLimits: {
    timeoutMs: 3000,
    maxSourceBytes: 65536,
    maxInputBytes: 262144,
    maxOutputBytes: 262144,
  },
};

export default validParentheses;

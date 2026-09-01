import type { Problem } from "../types";

const containsDuplicate: Problem = {
  id: "contains-duplicate",
  title: "Contains Duplicate",
  difficulty: "beginner",
  topics: ["arrays", "hash-maps"],
  statement: `Given an integer array \`nums\`, return \`true\` if any value appears at least twice in the array, and \`false\` if every element is distinct.`,
  constraints: [
    "1 <= nums.length <= 10^5",
    "-10^9 <= nums[i] <= 10^9",
  ],
  examples: [
    {
      input: "nums = [1, 2, 3, 1]",
      output: "true",
      explanation: "The value 1 appears twice.",
    },
    {
      input: "nums = [1, 2, 3, 4]",
      output: "false",
      explanation: "All values are distinct.",
    },
    {
      input: "nums = [1, 1, 1, 3, 3, 4, 3, 2, 4, 2]",
      output: "true",
      explanation: "Multiple repeated values.",
    },
  ],
  entryFunctionName: "containsDuplicate",
  starterCode: `function containsDuplicate(nums) {
  // TODO: return true if any value appears more than once
  return false;
}
`,
  visibleTests: [
    { id: "has_duplicate", args: [[1, 2, 3, 1]], expected: true },
    { id: "all_distinct", args: [[1, 2, 3, 4]], expected: false },
    { id: "many_duplicates", args: [[1, 1, 1, 3, 3, 4, 3, 2, 4, 2]], expected: true },
    { id: "single_element", args: [[7]], expected: false },
    { id: "two_same", args: [[5, 5]], expected: true },
    { id: "negatives", args: [[-1, -2, -3, -2]], expected: true },
  ],
  evaluatorVersion: "1.0.0",
  executionLimits: {
    timeoutMs: 3000,
    maxSourceBytes: 65536,
    maxInputBytes: 262144,
    maxOutputBytes: 262144,
  },
};

export default containsDuplicate;

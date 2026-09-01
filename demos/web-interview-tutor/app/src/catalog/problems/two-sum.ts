import type { Problem } from "../types";

const twoSum: Problem = {
  id: "two-sum",
  title: "Two Sum",
  difficulty: "beginner",
  topics: ["arrays", "hash-maps"],
  statement: `Given an array of integers \`nums\` and an integer \`target\`, return indices of the two numbers such that they add up to \`target\`.

You may assume that each input has exactly one solution, and you may not use the same element twice.

Return the two indices in ascending order.`,
  constraints: [
    "2 <= nums.length <= 10^4",
    "-10^9 <= nums[i] <= 10^9",
    "-10^9 <= target <= 10^9",
    "Exactly one valid answer exists.",
  ],
  examples: [
    {
      input: "nums = [2, 7, 11, 15], target = 9",
      output: "[0, 1]",
      explanation: "2 + 7 = 9, so indices 0 and 1 are returned.",
    },
    {
      input: "nums = [3, 2, 4], target = 6",
      output: "[1, 2]",
      explanation: "2 + 4 = 6, so indices 1 and 2 are returned.",
    },
    {
      input: "nums = [3, 3], target = 6",
      output: "[0, 1]",
      explanation: "3 + 3 = 6, the two equal elements are at indices 0 and 1.",
    },
  ],
  entryFunctionName: "twoSum",
  starterCode: `function twoSum(nums, target) {
  // TODO: return the indices of the two numbers that add to target
  return [];
}
`,
  visibleTests: [
    { id: "basic", args: [[2, 7, 11, 15], 9], expected: [0, 1] },
    { id: "unordered", args: [[3, 2, 4], 6], expected: [1, 2] },
    { id: "duplicates", args: [[3, 3], 6], expected: [0, 1] },
    { id: "negatives", args: [[-1, -2, -3, -4, -5], -8], expected: [2, 4] },
    { id: "zero_and_positive", args: [[0, 4, 3, 0], 0], expected: [0, 3] },
  ],
  evaluatorVersion: "1.0.0",
  executionLimits: {
    timeoutMs: 3000,
    maxSourceBytes: 65536,
    maxInputBytes: 262144,
    maxOutputBytes: 262144,
  },
};

export default twoSum;

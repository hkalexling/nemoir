import type { Problem } from "../types";

const binarySearch: Problem = {
  id: "binary-search",
  title: "Binary Search",
  difficulty: "beginner",
  topics: ["binary-search"],
  statement: `Given a **sorted** (ascending) array of integers \`nums\` and an integer \`target\`, return the index of \`target\` in \`nums\`.

If \`target\` is not present, return \`-1\`.

You must write an algorithm with O(log n) runtime complexity.`,
  constraints: [
    "1 <= nums.length <= 10^4",
    "-10^4 <= nums[i] <= 10^4",
    "All values in nums are unique.",
    "nums is sorted in ascending order.",
  ],
  examples: [
    {
      input: "nums = [-1, 0, 3, 5, 9, 12], target = 9",
      output: "4",
      explanation: "9 is at index 4 in the sorted array.",
    },
    {
      input: "nums = [-1, 0, 3, 5, 9, 12], target = 2",
      output: "-1",
      explanation: "2 is not present in the array.",
    },
  ],
  entryFunctionName: "binarySearch",
  starterCode: `function binarySearch(nums, target) {
  // TODO: implement O(log n) binary search
  return -1;
}
`,
  visibleTests: [
    { id: "found_middle", args: [[-1, 0, 3, 5, 9, 12], 9], expected: 4 },
    { id: "not_found", args: [[-1, 0, 3, 5, 9, 12], 2], expected: -1 },
    { id: "first_element", args: [[1, 2, 3, 4, 5], 1], expected: 0 },
    { id: "last_element", args: [[1, 2, 3, 4, 5], 5], expected: 4 },
    { id: "single_element_match", args: [[7], 7], expected: 0 },
    { id: "single_element_miss", args: [[7], 3], expected: -1 },
    { id: "negative_target", args: [[-5, -3, 0, 2, 8], -3], expected: 1 },
  ],
  evaluatorVersion: "1.0.0",
  executionLimits: {
    timeoutMs: 3000,
    maxSourceBytes: 65536,
    maxInputBytes: 262144,
    maxOutputBytes: 262144,
  },
};

export default binarySearch;

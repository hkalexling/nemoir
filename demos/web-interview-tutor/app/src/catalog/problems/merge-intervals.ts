import type { Problem } from "../types";

const mergeIntervals: Problem = {
  id: "merge-intervals",
  title: "Merge Intervals",
  difficulty: "intermediate",
  topics: ["intervals"],
  statement: `Given an array of \`intervals\` where \`intervals[i] = [start_i, end_i]\`, merge all overlapping intervals and return an array of the non-overlapping intervals that cover all the intervals in the input.

Two intervals overlap when one starts before the other ends (or at the same point).`,
  constraints: [
    "1 <= intervals.length <= 10^4",
    "intervals[i].length == 2",
    "0 <= start_i <= end_i <= 10^4",
  ],
  examples: [
    {
      input: "intervals = [[1, 3], [2, 6], [8, 10], [15, 18]]",
      output: "[[1, 6], [8, 10], [15, 18]]",
      explanation: "[1, 3] and [2, 6] overlap, so they merge into [1, 6].",
    },
    {
      input: "intervals = [[1, 4], [4, 5]]",
      output: "[[1, 5]]",
      explanation: "[1, 4] and [4, 5] touch at 4, so they merge into [1, 5].",
    },
  ],
  entryFunctionName: "mergeIntervals",
  starterCode: `function mergeIntervals(intervals) {
  // TODO: return the merged non-overlapping intervals
  return [];
}
`,
  visibleTests: [
    {
      id: "basic_overlap",
      args: [[[1, 3], [2, 6], [8, 10], [15, 18]]],
      expected: [[1, 6], [8, 10], [15, 18]],
    },
    {
      id: "touching",
      args: [[[1, 4], [4, 5]]],
      expected: [[1, 5]],
    },
    {
      id: "no_overlap",
      args: [[[1, 2], [3, 4], [5, 6]]],
      expected: [[1, 2], [3, 4], [5, 6]],
    },
    {
      id: "all_merge",
      args: [[[1, 10], [2, 3], [4, 5], [6, 7], [8, 9]]],
      expected: [[1, 10]],
    },
    {
      id: "single_interval",
      args: [[[0, 0]]],
      expected: [[0, 0]],
    },
    {
      id: "unsorted_input",
      args: [[[8, 10], [1, 3], [15, 18], [2, 6]]],
      expected: [[1, 6], [8, 10], [15, 18]],
    },
  ],
  evaluatorVersion: "1.0.0",
  executionLimits: {
    timeoutMs: 3000,
    maxSourceBytes: 65536,
    maxInputBytes: 262144,
    maxOutputBytes: 262144,
  },
};

export default mergeIntervals;

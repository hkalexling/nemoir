import type { Problem } from "../types";

const numberOfIslands: Problem = {
  id: "number-of-islands",
  title: "Number of Islands",
  difficulty: "intermediate",
  topics: ["bfs", "dfs"],
  statement: `Given an \`m x n\` 2-D binary grid \`grid\` where \`"1"\` represents land and \`"0"\` represents water, return the number of islands.

An island is surrounded by water and is formed by connecting adjacent lands horizontally or vertically. You may assume all four edges of the grid are surrounded by water.`,
  constraints: [
    "m == grid.length",
    "n == grid[i].length",
    "1 <= m, n <= 300",
    "grid[i][j] is either \"0\" or \"1\".",
  ],
  examples: [
    {
      input: `grid = [
  ["1","1","1","1","0"],
  ["1","1","0","1","0"],
  ["1","1","0","0","0"],
  ["0","0","0","0","0"]
]`,
      output: "1",
      explanation: "All land cells are connected into a single island.",
    },
    {
      input: `grid = [
  ["1","1","0","0","0"],
  ["1","1","0","0","0"],
  ["0","0","1","0","0"],
  ["0","0","0","1","1"]
]`,
      output: "3",
      explanation: "Three separate islands: top-left 2x2, middle single cell, bottom-right 1x2.",
    },
  ],
  entryFunctionName: "numIslands",
  starterCode: `function numIslands(grid) {
  // TODO: return the number of islands in the 2-D grid
  return 0;
}
`,
  visibleTests: [
    {
      id: "single_island",
      args: [
        [
          ["1", "1", "1", "1", "0"],
          ["1", "1", "0", "1", "0"],
          ["1", "1", "0", "0", "0"],
          ["0", "0", "0", "0", "0"],
        ],
      ],
      expected: 1,
    },
    {
      id: "three_islands",
      args: [
        [
          ["1", "1", "0", "0", "0"],
          ["1", "1", "0", "0", "0"],
          ["0", "0", "1", "0", "0"],
          ["0", "0", "0", "1", "1"],
        ],
      ],
      expected: 3,
    },
    {
      id: "no_land",
      args: [
        [
          ["0", "0"],
          ["0", "0"],
        ],
      ],
      expected: 0,
    },
    {
      id: "all_land",
      args: [
        [
          ["1", "1"],
          ["1", "1"],
        ],
      ],
      expected: 1,
    },
    {
      id: "diagonal_water",
      args: [
        [
          ["1", "0", "1"],
          ["0", "1", "0"],
          ["1", "0", "1"],
        ],
      ],
      expected: 5,
    },
    {
      id: "single_cell_land",
      args: [[["1"]]],
      expected: 1,
    },
  ],
  evaluatorVersion: "1.0.0",
  executionLimits: {
    timeoutMs: 5000,
    maxSourceBytes: 65536,
    maxInputBytes: 262144,
    maxOutputBytes: 262144,
  },
};

export default numberOfIslands;

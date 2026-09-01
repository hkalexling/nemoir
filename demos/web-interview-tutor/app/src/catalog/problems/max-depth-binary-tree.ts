import type { JsonValue, Problem } from "../types";

/**
 * A binary-tree node represented as a plain JSON-compatible object.
 *
 *     { val: 3, left: { val: 9, left: null, right: null }, right: null }
 *
 * `null` means the node (or child) is absent.
 */
interface TreeNode {
  readonly val: number;
  readonly left: TreeNode | null;
  readonly right: TreeNode | null;
}

const maxDepthBinaryTree: Problem = {
  id: "max-depth-binary-tree",
  title: "Maximum Depth of Binary Tree",
  difficulty: "intermediate",
  topics: ["trees", "dfs"],
  statement: `Given the root of a binary tree, return its maximum depth.

The maximum depth is the number of nodes along the longest path from the root node down to the farthest leaf node.

A leaf is a node with no children.

The tree is passed as a plain object: each node has \`val\` (number), \`left\` (node or \`null\`), and \`right\` (node or \`null\`). An empty tree is represented by \`null\`.`,
  constraints: [
    "The number of nodes in the tree is in the range [0, 10^4].",
    "-100 <= Node.val <= 100",
  ],
  examples: [
    {
      input: "root = { val: 3, left: { val: 9, left: null, right: null }, right: { val: 20, left: { val: 15, left: null, right: null }, right: { val: 7, left: null, right: null } } }",
      output: "3",
      explanation: "The longest path is 3 -> 20 -> 15 (or 3 -> 20 -> 7), producing a depth of 3.",
    },
    {
      input: "root = { val: 1, left: null, right: { val: 2, left: null, right: null } }",
      output: "2",
      explanation: "Root (1) followed by its right child (2) gives depth 2.",
    },
  ],
  entryFunctionName: "maxDepth",
  starterCode: `function maxDepth(root) {
  // TODO: return the maximum depth of the binary tree
  return 0;
}
`,
  visibleTests: [
    {
      id: "three_level",
      args: [
        {
          val: 3,
          left: { val: 9, left: null, right: null },
          right: {
            val: 20,
            left: { val: 15, left: null, right: null },
            right: { val: 7, left: null, right: null },
          },
        } satisfies TreeNode as unknown as JsonValue,
      ],
      expected: 3,
    },
    {
      id: "right_skewed",
      args: [
        {
          val: 1,
          left: null,
          right: { val: 2, left: null, right: null },
        } satisfies TreeNode as unknown as JsonValue,
      ],
      expected: 2,
    },
    {
      id: "empty_tree",
      args: [null],
      expected: 0,
    },
    {
      id: "single_node",
      args: [{ val: 42, left: null, right: null } satisfies TreeNode as unknown as JsonValue],
      expected: 1,
    },
    {
      id: "balanced_two",
      args: [
        {
          val: 1,
          left: { val: 2, left: null, right: null },
          right: { val: 3, left: null, right: null },
        } satisfies TreeNode as unknown as JsonValue,
      ],
      expected: 2,
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

export default maxDepthBinaryTree;

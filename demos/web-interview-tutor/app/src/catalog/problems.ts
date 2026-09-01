// ---------------------------------------------------------------------------
// Phase 2 problem catalog — index / aggregate module.
//
// Imports every curated problem, validates the full catalog, and exports a
// stable, read-only array.  Validation runs at module-load time so that
// catalog integrity errors surface early in development.
// ---------------------------------------------------------------------------

import type { Problem } from "./types";
import { isCatalogValid, validateCatalog } from "./types";

// ---- problem imports ------------------------------------------------------
import twoSum from "./problems/two-sum";
import validParentheses from "./problems/valid-parentheses";
import binarySearch from "./problems/binary-search";
import validPalindrome from "./problems/valid-palindrome";
import containsDuplicate from "./problems/contains-duplicate";
import mergeIntervals from "./problems/merge-intervals";
import maxDepthBinaryTree from "./problems/max-depth-binary-tree";
import numberOfIslands from "./problems/number-of-islands";

// ---- catalog assembly -----------------------------------------------------

/** Ordered catalog — kept as a const tuple for stable identity. */
export const PROBLEMS: readonly Problem[] = Object.freeze([
  twoSum,
  validParentheses,
  binarySearch,
  validPalindrome,
  containsDuplicate,
  mergeIntervals,
  maxDepthBinaryTree,
  numberOfIslands,
]) as readonly Problem[];

// ---- eager validation -----------------------------------------------------

const { results: _validationResults, catalogErrors: _catalogErrors } =
  validateCatalog(PROBLEMS);

if (!isCatalogValid(PROBLEMS)) {
  // Log per-problem errors.
  for (const r of _validationResults) {
    if (!r.valid) {
      console.error(
        `[catalog] Problem "${r.id}" validation errors:`,
        r.errors,
      );
    }
  }
  // Log cross-problem errors.
  for (const err of _catalogErrors) {
    console.error(`[catalog] ${err}`);
  }
  throw new Error(
    `Catalog validation failed with ${_validationResults.filter((r) => !r.valid).length} problem error(s) and ${_catalogErrors.length} cross-problem error(s). See console for details.`,
  );
}

// ---- lookup helpers -------------------------------------------------------

const _byId = new Map<string, Problem>(
  PROBLEMS.map((p) => [p.id, p]),
);

/** O(1) lookup by problem id.  Returns `undefined` when not found. */
export function getProblemById(id: string): Problem | undefined {
  return _byId.get(id);
}

/** All distinct topics present in the catalog, sorted alphabetically. */
export const ALL_TOPICS: readonly string[] = Object.freeze(
  Array.from(new Set(PROBLEMS.flatMap((p) => p.topics))).sort(),
);

/** Filter problems by a single topic. */
export function getProblemsByTopic(topic: string): readonly Problem[] {
  return PROBLEMS.filter((p) => p.topics.includes(topic));
}

/** Filter problems by difficulty level. */
export function getProblemsByDifficulty(
  difficulty: Problem["difficulty"],
): readonly Problem[] {
  return PROBLEMS.filter((p) => p.difficulty === difficulty);
}

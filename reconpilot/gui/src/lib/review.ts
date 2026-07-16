import type { ReviewItemLike } from "../types";

export interface ReviewFilters {
  search: string;
  riskLevel: string;
  role: string;
  environment: string;
  sortBy: "rank" | "score" | "confidence";
}

export function collectReviewFacets(items: ReviewItemLike[]): {
  riskLevels: string[];
  roles: string[];
  environments: string[];
} {
  return {
    riskLevels: uniqueSorted(items.map((item) => item.risk_level)),
    roles: uniqueSorted(items.flatMap((item) => item.semantic_roles)),
    environments: uniqueSorted(items.flatMap((item) => item.environments))
  };
}

export function filterReviewItems(
  items: ReviewItemLike[],
  filters: ReviewFilters
): ReviewItemLike[] {
  const search = filters.search.trim().toLowerCase();

  const filtered = items.filter((item) => {
    if (filters.riskLevel !== "all" && item.risk_level !== filters.riskLevel) {
      return false;
    }
    if (filters.role !== "all" && !item.semantic_roles.includes(filters.role)) {
      return false;
    }
    if (
      filters.environment !== "all" &&
      !item.environments.includes(filters.environment)
    ) {
      return false;
    }
    if (!search) {
      return true;
    }

    return [
      item.asset,
      item.risk_level,
      ...item.semantic_roles,
      ...item.environments,
      ...item.reasons
    ]
      .join(" ")
      .toLowerCase()
      .includes(search);
  });

  return filtered.sort((left, right) => {
    switch (filters.sortBy) {
      case "score":
        return right.score - left.score || left.rank - right.rank;
      case "confidence":
        return right.confidence - left.confidence || left.rank - right.rank;
      case "rank":
      default:
        return left.rank - right.rank;
    }
  });
}

function uniqueSorted(values: string[]): string[] {
  return [...new Set(values.filter(Boolean))].sort((left, right) =>
    left.localeCompare(right)
  );
}

import { useCallback } from "react";
import type { Observation } from "@trytilde/contracts/tilde/management/v1/tracing_pb.js";
import { FilterSearchInput } from "../observability/filter-search-input";
import { parseQuery, serializeQuery, suggestions } from "./filter-query";
import type { TraceSearch } from "./search";
export function TraceSearchInput({
  search,
  observations,
  onApply,
}: {
  search: TraceSearch;
  observations: Observation[];
  onApply: (patch: TraceSearch) => void;
}) {
  const suggest = useCallback(
    (query: string, caret: number) => suggestions(query, caret, observations),
    [observations],
  );
  return (
    <FilterSearchInput
      label="Search traces"
      canonical={serializeQuery(search)}
      validate={parseQuery}
      suggest={suggest}
      onApply={(query) => {
        const patch = parseQuery(query);
        onApply({ ...patch, preset: patch.type || patch.level ? "all" : search.preset });
      }}
    />
  );
}

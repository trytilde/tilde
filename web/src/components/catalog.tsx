import { useId, useState, type ReactNode } from "react";
import { ChevronDownIcon, SearchIcon, TagIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

/*
 * Catalog primitives shared by the Tilde skill catalog and the tool catalog, reproduced from
 * trytilde/dispatch (plugins-catalog.tsx): entries sectioned by category, a category filter and
 * search in the trace filter bar's style, and a 45px icon tile per entry.
 */

/** What a catalog row shows. */
type CatalogEntry = { id: string; name: string; description: string; iconUrl?: string };

export function categoryLabel(value: string) {
  const text = value.replaceAll(/[_-]+/g, " ");
  return text.charAt(0).toUpperCase() + text.slice(1);
}
export function compareCategories(left: string, right: string) {
  const leftIsOther = left.trim().toLowerCase() === "other";
  const rightIsOther = right.trim().toLowerCase() === "other";
  if (leftIsOther !== rightIsOther) return leftIsOther ? 1 : -1;
  return left.localeCompare(right);
}
function capabilityMark(name: string) {
  return name
    .split(/\s+/)
    .map((word) => word[0])
    .join("")
    .slice(0, 3)
    .toUpperCase();
}
function capabilityColor(id: string) {
  let hash = 0;
  for (const character of id) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 48% 43%)`;
}
/** 45px provider tile (28px `small`); letters on a colour hashed from the id when there is no
 * usable image. */
export function CatalogIcon({
  id,
  name,
  iconUrl,
  small = false,
}: {
  id: string;
  name: string;
  iconUrl?: string;
  small?: boolean;
}) {
  const [failed, setFailed] = useState<string>();
  const image = iconUrl && iconUrl !== failed ? iconUrl : undefined;
  return (
    <span
      aria-hidden="true"
      data-slot="provider-icon"
      className={cn(
        "grid shrink-0 place-items-center font-bold tracking-[-0.02em] text-white",
        small ? "size-7 rounded-md text-[9px]" : "size-[45px] rounded-[10px] text-[11px]",
        image && "bg-background dark:bg-white",
      )}
      style={image ? undefined : { backgroundColor: capabilityColor(id) }}
    >
      {image ? (
        <img
          alt=""
          className={cn(
            "h-auto w-auto object-contain",
            small ? "max-h-5 max-w-5" : "max-h-8 max-w-8",
          )}
          referrerPolicy="no-referrer"
          onError={() => setFailed(image)}
          src={image}
        />
      ) : (
        capabilityMark(name)
      )}
    </span>
  );
}

export function CatalogProviderRow({
  entry,
  onOpen,
  badge,
}: {
  entry: CatalogEntry;
  onOpen: () => void;
  /** Shown at the end of the row, such as an "Enabled" badge. */
  badge?: ReactNode;
}) {
  return (
    <li className="min-w-0">
      <button
        className="flex w-full min-w-0 cursor-pointer items-center gap-3 rounded-2xl bg-transparent px-3 py-[9.5px] text-left hover:bg-foreground/5 focus-visible:bg-foreground/5 focus-visible:outline-none"
        onClick={onOpen}
        type="button"
      >
        <CatalogIcon id={entry.id} name={entry.name} iconUrl={entry.iconUrl} />
        <div className="flex min-w-0 flex-1 flex-col gap-px">
          <h3 className="m-0 truncate text-[13px] leading-[18px] font-medium text-foreground">
            {entry.name}
          </h3>
          <p className="m-0 truncate text-[13px] leading-[18px] text-foreground/60">
            {entry.description}
          </p>
        </div>
        {badge}
      </button>
    </li>
  );
}

export function CatalogSection({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section aria-labelledby={id}>
      <h3
        className="m-0 px-2 pt-2 pb-1.5 text-[13px] leading-[18px] font-medium text-foreground/40"
        id={id}
      >
        {title}
      </h3>
      <ul className="m-0 grid list-none grid-cols-2 gap-x-2 gap-y-0.5 p-0 max-[980px]:grid-cols-1">
        {children}
      </ul>
    </section>
  );
}

// Filter controls follow the trace filter bar: 26px square-edged triggers and menus in 11px mono.
const filterTrigger =
  "trace-controls flex h-[26px] shrink-0 cursor-pointer items-center gap-2 rounded-none border border-border bg-background px-2 text-[11px] shadow-none outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/40 data-popup-open:bg-muted";
const menuContent =
  "trace-controls rounded-none border border-border bg-popover p-0 shadow-lg ring-0";
const menuItem = "h-7 rounded-none border-b border-border/50 px-2 text-[11px] last:border-b-0";

export function CategoryFilter({
  categories,
  selectedCategory,
  onSelect,
}: {
  categories: readonly string[];
  selectedCategory: string | null;
  onSelect: (category: string | null) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label={selectedCategory ? `Category: ${categoryLabel(selectedCategory)}` : "Category"}
        className={filterTrigger}
      >
        <TagIcon aria-hidden="true" className="size-3 text-muted-foreground" />
        <span className="max-w-32 truncate">
          {selectedCategory ? categoryLabel(selectedCategory) : "Category"}
        </span>
        <ChevronDownIcon aria-hidden="true" className="size-3 text-muted-foreground" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className={cn("min-w-[200px]", menuContent)}>
        <DropdownMenuItem className={menuItem} onClick={() => onSelect(null)}>
          All categories
        </DropdownMenuItem>
        <DropdownMenuSeparator className="-mx-0 my-0" />
        {categories.map((category) => (
          <DropdownMenuCheckboxItem
            checked={selectedCategory === category}
            className={cn(menuItem, "pl-8")}
            closeOnClick
            key={category}
            onCheckedChange={() => onSelect(selectedCategory === category ? null : category)}
          >
            {categoryLabel(category)}
          </DropdownMenuCheckboxItem>
        ))}
        {categories.length === 0 ? (
          <DropdownMenuItem className={menuItem} disabled>
            No categories available
          </DropdownMenuItem>
        ) : null}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export function CatalogSearchField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (query: string) => void;
}) {
  return (
    <label className="trace-controls relative flex h-[26px] min-w-40 flex-1 items-center rounded-md border border-input bg-background focus-within:ring-2 focus-within:ring-ring/40 sm:max-w-sm">
      <SearchIcon
        aria-hidden="true"
        className="pointer-events-none absolute left-1.5 size-3 text-muted-foreground"
      />
      <span className="sr-only">{label}</span>
      <input
        className="h-full w-full min-w-0 bg-transparent pr-2 pl-6 text-[11px] text-foreground outline-none placeholder:text-muted-foreground"
        onChange={(event) => onChange(event.target.value)}
        placeholder={label}
        type="search"
        value={value}
      />
    </label>
  );
}

export function CatalogSkeleton({ label = "Loading tools" }: { label?: string }) {
  return (
    <div
      aria-label={label}
      className="grid grid-cols-2 gap-x-2 gap-y-0.5 max-[980px]:grid-cols-1"
      role="status"
    >
      {Array.from({ length: 6 }, (_, index) => (
        <div
          aria-hidden="true"
          className="flex h-16 min-w-0 animate-pulse items-center gap-3 rounded-2xl px-3 py-[9.5px] motion-reduce:animate-none"
          key={index}
        >
          <span className="size-[45px] shrink-0 rounded-[10px] bg-foreground/10" />
          <span className="min-w-0 flex-1 space-y-2">
            <span
              className={cn(
                "block h-2.5 rounded-full bg-foreground/10",
                index % 3 === 0 ? "w-28" : index % 3 === 1 ? "w-36" : "w-24",
              )}
            />
            <span className="block h-2 w-[min(90%,240px)] rounded-full bg-foreground/5" />
          </span>
          <span className="flex -space-x-1.5 pl-2">
            <span className="size-[30px] rounded-full border-2 border-background bg-foreground/10" />
            <span className="size-[30px] rounded-full border-2 border-background bg-muted" />
          </span>
        </div>
      ))}
    </div>
  );
}

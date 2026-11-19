"use client";

import { ArrowDownIcon, ArrowUpIcon, ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import type { ReactNode } from "react";

import { Empty } from "@/components/Empty/Empty";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { formatNumber } from "@/lib/format";
import { cn } from "@/lib/utils";

export interface Column<T> {
  key: string;
  header: string;
  cell: (row: T) => ReactNode;
  /** Right-aligned with tabular figures. */
  numeric?: boolean;
  /** Hidden below `md`. */
  hideOnMobile?: boolean;
  /** The sort this header switches between: [ascending, descending]. */
  sort?: [string, string];
}

interface Props<T> {
  columns: Column<T>[];
  rows: T[] | undefined;
  rowKey: (row: T) => string;
  loading?: boolean;
  empty: { title: string; description?: string };
  sort?: string;
  onSort?: (sort: string) => void;
  onRowClick?: (row: T) => void;
  /** `totalCapped`: more rows match than `total`; the total shows as "10,000+". */
  page?: {
    page: number;
    perPage: number;
    total: number;
    totalCapped?: boolean;
    onPage: (page: number) => void;
  };
  caption: string;
}

/**
 * A table with sorting, paging, a loading state at its final size, and an empty state. The
 * table scrolls sideways inside its own box on a narrow screen; the page never does.
 */
export function DataTable<T>({
  columns,
  rows,
  rowKey,
  loading,
  empty,
  sort,
  onSort,
  onRowClick,
  page,
  caption,
}: Props<T>) {
  const visible = (c: Column<T>) =>
    cn(c.hideOnMobile && "hidden md:table-cell", c.numeric && "text-right tabular-nums");
  const showSkeleton = loading && !rows;

  return (
    <div className="grid gap-3">
      <div className="overflow-x-auto rounded-lg border">
        <Table>
          <caption className="sr-only">{caption}</caption>
          <TableHeader>
            <TableRow>
              {columns.map((c) => {
                const [asc, desc] = c.sort ?? [];
                const dir = sort === asc ? "asc" : sort === desc ? "desc" : null;
                return (
                  <TableHead
                    key={c.key}
                    className={visible(c)}
                    aria-sort={
                      dir === "asc" ? "ascending" : dir === "desc" ? "descending" : undefined
                    }
                  >
                    {c.sort && onSort ? (
                      <button
                        type="button"
                        className="inline-flex items-center gap-1 hover:text-foreground"
                        onClick={() => onSort(dir === "asc" && desc ? desc : (asc ?? ""))}
                      >
                        {c.header}
                        {dir === "asc" ? <ArrowUpIcon className="size-3" /> : null}
                        {dir === "desc" ? <ArrowDownIcon className="size-3" /> : null}
                      </button>
                    ) : (
                      c.header
                    )}
                  </TableHead>
                );
              })}
            </TableRow>
          </TableHeader>
          <TableBody>
            {showSkeleton
              ? Array.from({ length: 5 }, (_, i) => (
                  <TableRow key={i}>
                    {columns.map((c) => (
                      <TableCell key={c.key} className={visible(c)}>
                        <Skeleton className="h-4 w-24" />
                      </TableCell>
                    ))}
                  </TableRow>
                ))
              : rows?.map((row) => (
                  <TableRow
                    key={rowKey(row)}
                    className={cn(onRowClick && "cursor-pointer")}
                    onClick={onRowClick ? () => onRowClick(row) : undefined}
                  >
                    {columns.map((c) => (
                      <TableCell key={c.key} className={visible(c)}>
                        {c.cell(row)}
                      </TableCell>
                    ))}
                  </TableRow>
                ))}
          </TableBody>
        </Table>
        {!showSkeleton && rows?.length === 0 ? (
          <Empty title={empty.title} description={empty.description} />
        ) : null}
      </div>
      {page && page.total > 0 ? <Pager {...page} /> : null}
    </div>
  );
}

function Pager({ page, perPage, total, totalCapped, onPage }: NonNullable<Props<unknown>["page"]>) {
  const last = Math.max(1, Math.ceil(total / perPage));
  const from = (page - 1) * perPage + 1;
  const to = Math.min(total, page * perPage);
  return (
    <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
      <p>
        {formatNumber(from)}–{formatNumber(to)} of {formatNumber(total)}
        {totalCapped ? "+" : ""}
      </p>
      <div className="flex gap-1">
        <Button
          variant="outline"
          size="icon-sm"
          aria-label="Previous page"
          disabled={page <= 1}
          onClick={() => onPage(page - 1)}
        >
          <ChevronLeftIcon />
        </Button>
        <Button
          variant="outline"
          size="icon-sm"
          aria-label="Next page"
          disabled={page >= last}
          onClick={() => onPage(page + 1)}
        >
          <ChevronRightIcon />
        </Button>
      </div>
    </div>
  );
}

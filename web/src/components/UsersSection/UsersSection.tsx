"use client";

import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { PlusIcon, SearchIcon } from "lucide-react";
import type { Route } from "next";
import { useRouter } from "next/navigation";
import { useRef, useState } from "react";

import { CreateUserModal } from "@/components/CreateUserModal/CreateUserModal";
import { type Column, DataTable } from "@/components/DataTable/DataTable";
import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { StatusBadge } from "@/components/StatusBadge/StatusBadge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api, call, type Schemas } from "@/lib/api/client";
import { formatDate, formatName, formatRelative } from "@/lib/format";
import { ROLE_LABEL, STATUS_LABEL } from "@/lib/labels";
import { keys } from "@/lib/queries";

type User = Schemas["UserDto"];
type Sort = Schemas["UserSort"];
const PER_PAGE = 20;
const SEARCH_DELAY_MS = 300;

const ROLE_ITEMS = { all: "All roles", ...ROLE_LABEL };
const STATUS_ITEMS = { all: "All statuses", ...STATUS_LABEL };

const columns: Column<User>[] = [
  {
    key: "name",
    header: "Name",
    sort: ["name_asc", "name_desc"],
    cell: (u) => (
      <div className="min-w-0">
        <p className="truncate font-medium">{formatName(u)}</p>
        <p className="truncate text-xs text-muted-foreground md:hidden">{u.email}</p>
      </div>
    ),
  },
  {
    key: "email",
    header: "Email",
    hideOnMobile: true,
    sort: ["email_asc", "email_asc"],
    cell: (u) => u.email,
  },
  { key: "role", header: "Role", cell: (u) => <StatusBadge value={u.role} /> },
  { key: "status", header: "Status", cell: (u) => <StatusBadge value={u.status} /> },
  {
    key: "last_login",
    header: "Last login",
    hideOnMobile: true,
    sort: ["last_login_desc", "last_login_desc"],
    cell: (u) => (
      <span className="text-xs text-muted-foreground">{formatRelative(u.last_login_at)}</span>
    ),
  },
  {
    key: "created",
    header: "Created",
    hideOnMobile: true,
    sort: ["created_asc", "created_desc"],
    cell: (u) => <span className="text-xs text-muted-foreground">{formatDate(u.created_at)}</span>,
  },
];

export function UsersSection({ canManage }: { canManage: boolean }) {
  const router = useRouter();
  const [input, setInput] = useState("");
  const [search, setSearch] = useState("");
  const [role, setRole] = useState<string>("all");
  const [status, setStatus] = useState<string>("all");
  const [sort, setSort] = useState<Sort>("created_desc");
  const [page, setPage] = useState(1);
  const [creating, setCreating] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const onSearch = (value: string) => {
    setInput(value);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      setSearch(value.trim());
      setPage(1);
    }, SEARCH_DELAY_MS);
  };

  const query = {
    search: search || undefined,
    role: role === "all" ? undefined : (role as Schemas["Role"]),
    status: status === "all" ? undefined : (status as Schemas["UserStatus"]),
    sort,
    page,
    per_page: PER_PAGE,
  };
  const users = useQuery({
    queryKey: keys.users(query),
    queryFn: () => call(api.GET("/api/users", { params: { query } })),
    placeholderData: keepPreviousData,
  });

  return (
    <>
      <div className="flex flex-wrap items-end justify-between gap-3 pb-3">
        <h2 id="users-heading" className="text-lg font-semibold">
          Users
        </h2>
        {canManage ? (
          <Button onClick={() => setCreating(true)}>
            <PlusIcon />
            Create user
          </Button>
        ) : null}
      </div>
      <div className="grid gap-2 pb-3 sm:grid-cols-[1fr_auto_auto]">
        <div className="relative">
          <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            type="search"
            aria-label="Search by name or email"
            placeholder="Search by name or email"
            value={input}
            onChange={(e) => onSearch(e.target.value)}
            className="pl-8"
          />
        </div>
        <Select
          items={ROLE_ITEMS}
          value={role}
          onValueChange={(v) => {
            setRole(String(v));
            setPage(1);
          }}
        >
          <SelectTrigger aria-label="Role" className="w-full sm:w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {Object.entries(ROLE_ITEMS).map(([value, label]) => (
              <SelectItem key={value} value={value}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select
          items={STATUS_ITEMS}
          value={status}
          onValueChange={(v) => {
            setStatus(String(v));
            setPage(1);
          }}
        >
          <SelectTrigger aria-label="Status" className="w-full sm:w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {Object.entries(STATUS_ITEMS).map(([value, label]) => (
              <SelectItem key={value} value={value}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {users.error ? (
        <ErrorAlert error={users.error} onRetry={() => void users.refetch()} />
      ) : (
        <DataTable
          caption="Users"
          columns={columns}
          rows={users.data?.items}
          rowKey={(u) => u.id}
          loading={users.isPending}
          empty={
            search || role !== "all" || status !== "all"
              ? { title: "No users match", description: "Change the search or the filters." }
              : { title: "No users yet" }
          }
          sort={sort}
          onSort={(s) => setSort(s as Sort)}
          onRowClick={(u) => router.push(`/users/${u.id}` as Route)}
          page={
            users.data
              ? {
                  page,
                  perPage: PER_PAGE,
                  total: users.data.total,
                  totalCapped: users.data.total_capped,
                  onPage: setPage,
                }
              : undefined
          }
        />
      )}
      {canManage ? <CreateUserModal open={creating} onOpenChange={setCreating} /> : null}
    </>
  );
}

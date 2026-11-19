"use client";

import { PageBar } from "@/components/PageBar/PageBar";
import { StatCard } from "@/components/StatCard/StatCard";
import { StatusBadge } from "@/components/StatusBadge/StatusBadge";
import { UsersSection } from "@/components/UsersSection/UsersSection";
import { formatDate, formatRelative } from "@/lib/format";
import { useMe } from "@/lib/queries";

export function DashboardView() {
  const { data: me } = useMe();
  if (!me) return null;
  const canRead = me.permissions.includes("users.read");
  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-4 md:px-5">
      <PageBar title={`Hello, ${me.user.first_name}`} description="Your account at a glance." />
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatCard label="Role" value={<StatusBadge value={me.user.role} />} />
        <StatCard label="Status" value={<StatusBadge value={me.user.status} />} />
        <StatCard label="Member since" value={formatDate(me.user.created_at)} />
        <StatCard label="Last login" value={formatRelative(me.user.last_login_at)} />
      </div>
      {canRead ? (
        <section className="mt-8" aria-labelledby="users-heading">
          <UsersSection canManage={me.permissions.includes("users.manage")} />
        </section>
      ) : null}
    </div>
  );
}

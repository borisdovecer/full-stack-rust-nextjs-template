import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { type Column, DataTable } from "./DataTable";

type Row = { id: string; name: string };
const columns: Column<Row>[] = [
  { key: "name", header: "Name", cell: (r) => r.name, sort: ["name_asc", "name_desc"] },
];

describe("DataTable", () => {
  it("shows rows and pages", async () => {
    const onPage = vi.fn();
    render(
      <DataTable
        caption="Users"
        columns={columns}
        rows={[{ id: "1", name: "Ana" }]}
        rowKey={(r) => r.id}
        empty={{ title: "No users" }}
        page={{ page: 1, perPage: 1, total: 3, onPage }}
      />,
    );
    expect(screen.getByText("Ana")).toBeInTheDocument();
    expect(screen.getByText(/1–1 of 3/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous page" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Next page" }));
    expect(onPage).toHaveBeenCalledWith(2);
  });

  it("marks a capped total and stops at it", () => {
    render(
      <DataTable
        caption="Users"
        columns={columns}
        rows={[{ id: "1", name: "Ana" }]}
        rowKey={(r) => r.id}
        empty={{ title: "No users" }}
        page={{ page: 500, perPage: 20, total: 10_000, totalCapped: true, onPage: vi.fn() }}
      />,
    );
    expect(screen.getByText(/9,981–10,000 of 10,000\+/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next page" })).toBeDisabled();
  });

  it("shows the empty state", () => {
    render(
      <DataTable
        caption="Users"
        columns={columns}
        rows={[]}
        rowKey={(r) => r.id}
        empty={{ title: "No users" }}
      />,
    );
    expect(screen.getByText("No users")).toBeInTheDocument();
  });

  it("switches sort direction from the header", async () => {
    const onSort = vi.fn();
    const { rerender } = render(
      <DataTable
        caption="Users"
        columns={columns}
        rows={[]}
        rowKey={(r) => r.id}
        empty={{ title: "x" }}
        sort="created_desc"
        onSort={onSort}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Name" }));
    expect(onSort).toHaveBeenLastCalledWith("name_asc");
    rerender(
      <DataTable
        caption="Users"
        columns={columns}
        rows={[]}
        rowKey={(r) => r.id}
        empty={{ title: "x" }}
        sort="name_asc"
        onSort={onSort}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Name" }));
    expect(onSort).toHaveBeenLastCalledWith("name_desc");
  });
});

"use client";

import { InboxIcon, InfoIcon } from "lucide-react";
import { type ReactNode, useState } from "react";
import { toast } from "sonner";

import { CodeField, SendAgain } from "@/components/CodeField/CodeField";
import { ConfirmModal } from "@/components/ConfirmModal/ConfirmModal";
import { CopyValue } from "@/components/CopyValue/CopyValue";
import { type Column, DataTable } from "@/components/DataTable/DataTable";
import { Empty } from "@/components/Empty/Empty";
import { ErrorAlert } from "@/components/ErrorAlert/ErrorAlert";
import { Fact } from "@/components/Fact/Fact";
import { Field, TextField, TextareaField } from "@/components/Field/Field";
import { FormActions } from "@/components/FormActions/FormActions";
import { Markdown } from "@/components/Markdown/Markdown";
import { MethodSwitch } from "@/components/MethodSwitch/MethodSwitch";
import { Modal } from "@/components/Modal/Modal";
import { PageBar } from "@/components/PageBar/PageBar";
import { PasswordInput } from "@/components/PasswordInput/PasswordInput";
import { ProviderButtons } from "@/components/ProviderButtons/ProviderButtons";
import { Spinner } from "@/components/Spinner/Spinner";
import { StatCard } from "@/components/StatCard/StatCard";
import { StatusBadge } from "@/components/StatusBadge/StatusBadge";
import { ThemeToggle } from "@/components/ThemeToggle/ThemeToggle";
import { Tip } from "@/components/Tip/Tip";
import { WalletConnect } from "@/components/WalletConnect/WalletConnect";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ApiError, type Schemas } from "@/lib/api/client";

/**
 * Reusable components, each shown on this page. A new component folder goes either here (and
 * on the page) or into VIEWS; components.test.ts fails on a folder in neither.
 */
export const REUSABLE = [
  "AppShell",
  "AuthGate",
  "Header",
  "Sidebar",
  "BottomNav",
  "PageBar",
  "UserMenu",
  "ThemeToggle",
  "DataTable",
  "StatCard",
  "StatusBadge",
  "Fact",
  "CopyValue",
  "Empty",
  "ErrorAlert",
  "Spinner",
  "Modal",
  "ConfirmModal",
  "Tip",
  "Field",
  "PasswordInput",
  "FormActions",
  "MethodSwitch",
  "CodeField",
  "WalletConnect",
  "ProviderButtons",
  "Markdown",
  "AuthCard",
  "AppName",
];

/** Page views and app plumbing: one use each, not meant for reuse. */
export const VIEWS = [
  "ComponentsShowcase",
  "CreateUserModal",
  "DashboardView",
  "HelpView",
  "LoginForm",
  "PendingView",
  "Providers",
  "RegisterForm",
  "SettingsView",
  "ThemeSync",
  "UserDetailView",
  "UsersSection",
  "Welcome",
];

type Row = { id: string; name: string; status: "active" | "pending" | "suspended"; logins: number };
const ROWS: Row[] = [
  { id: "1", name: "Ana Example", status: "active", logins: 1204 },
  { id: "2", name: "Bo Example", status: "pending", logins: 0 },
  { id: "3", name: "Cy Example", status: "suspended", logins: 37 },
];
const COLUMNS: Column<Row>[] = [
  { key: "name", header: "Name", cell: (r) => r.name, sort: ["name_asc", "name_desc"] },
  { key: "status", header: "Status", cell: (r) => <StatusBadge value={r.status} /> },
  { key: "logins", header: "Logins", numeric: true, cell: (r) => r.logins.toLocaleString() },
];

const SAMPLE_MD = `## Markdown

Rendered from the API for the **Help** page. Supports [links](architecture.md), \`code\`, lists and tables.

| Token | Use |
|---|---|
| \`--primary\` | main actions |
| \`--muted\` | quiet surfaces |
`;

function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section id={id} aria-labelledby={`${id}-h`} className="grid scroll-mt-20 gap-4">
      <h2 id={`${id}-h`} className="text-lg font-semibold">
        {title}
      </h2>
      {children}
      <Separator className="mt-4" />
    </section>
  );
}

function Demo({ name, children }: { name: string; children: ReactNode }) {
  return (
    <div className="grid gap-2">
      <p className="font-mono text-xs text-muted-foreground">{name}</p>
      <div className="rounded-lg border p-4">{children}</div>
    </div>
  );
}

export function ComponentsShowcase() {
  const [sort, setSort] = useState("name_asc");
  const [page, setPage] = useState(1);
  const [modal, setModal] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [checked, setChecked] = useState(true);
  const [select, setSelect] = useState("one");
  const [method, setMethod] = useState<Schemas["LoginType"]>("password");
  const [code, setCode] = useState("");

  return (
    <div className="mx-auto grid w-full max-w-5xl gap-8 px-4 py-6 md:px-5">
      <PageBar
        title="Components"
        description="Every reusable component in its states. Development only."
        actions={<ThemeToggle />}
      />

      <Section id="shell" title="Shell">
        <p className="text-muted-foreground">
          AppShell, Header, Sidebar, BottomNav, UserMenu, AuthGate, AuthCard and AppName frame every
          page: see any signed-in page and the login page. PageBar is the title row above.
        </p>
      </Section>

      <Section id="data" title="Data">
        <Demo name="StatCard">
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <StatCard label="Users" value="1,204" hint="+12 this week" />
            <StatCard label="Active" value="1,180" />
            <StatCard label="Status" value={<StatusBadge value="active" />} />
            <StatCard label="Unknown" value="—" />
          </div>
        </Demo>
        <Demo name="StatusBadge">
          <div className="flex flex-wrap gap-2">
            {(["active", "pending", "suspended", "admin", "user"] as const).map((v) => (
              <StatusBadge key={v} value={v} />
            ))}
          </div>
        </Demo>
        <Demo name="Fact, CopyValue">
          <dl className="grid gap-4 sm:grid-cols-3">
            <Fact label="Email">ana@example.com</Fact>
            <Fact label="Created">1 Jan 2030</Fact>
            <Fact label="Id">
              <CopyValue value="0b7e4a1c-2f6d-4c1e-9a3b-5d8e7f6a1b2c" />
            </Fact>
          </dl>
        </Demo>
        <Demo name="DataTable: rows, sorting, paging">
          <DataTable
            caption="Example"
            columns={COLUMNS}
            rows={ROWS}
            rowKey={(r) => r.id}
            empty={{ title: "No rows" }}
            sort={sort}
            onSort={setSort}
            page={{ page, perPage: 3, total: 9, onPage: setPage }}
          />
        </Demo>
        <Demo name="DataTable: loading">
          <DataTable
            caption="Loading"
            columns={COLUMNS}
            rows={undefined}
            loading
            rowKey={(r) => r.id}
            empty={{ title: "No rows" }}
          />
        </Demo>
        <Demo name="DataTable: empty">
          <DataTable
            caption="Empty"
            columns={COLUMNS}
            rows={[]}
            rowKey={(r) => r.id}
            empty={{ title: "No users match", description: "Change the search or the filters." }}
          />
        </Demo>
      </Section>

      <Section id="feedback" title="Feedback">
        <Demo name="Empty">
          <Empty
            icon={<InboxIcon />}
            title="Nothing here yet"
            description="Items you add show up here."
            action={<Button size="sm">Add one</Button>}
          />
        </Demo>
        <Demo name="ErrorAlert">
          <div className="grid gap-3">
            <ErrorAlert
              error={new ApiError("internal", 500, "a1b2c3d4")}
              onRetry={() => toast("Retrying")}
            />
            <ErrorAlert title="Could not save" error={new ApiError("network", 0, null)} />
          </div>
        </Demo>
        <Demo name="Alert">
          <Alert>
            <InfoIcon />
            <AlertTitle>Heads up</AlertTitle>
            <AlertDescription>A neutral message for something worth knowing.</AlertDescription>
          </Alert>
        </Demo>
        <Demo name="Skeleton, Spinner">
          <div className="flex items-center gap-4">
            <Skeleton className="h-10 w-40" />
            <Spinner />
            <Button disabled>
              <Spinner />
              Saving
            </Button>
          </div>
        </Demo>
        <Demo name="Toasts">
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" onClick={() => toast.success("Saved.")}>
              Success
            </Button>
            <Button variant="outline" onClick={() => toast.error("Could not save. Try again.")}>
              Error
            </Button>
            <Button variant="outline" onClick={() => toast("Something happened.")}>
              Neutral
            </Button>
          </div>
        </Demo>
      </Section>

      <Section id="overlays" title="Overlays">
        <Demo name="Modal, ConfirmModal, Tip">
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" onClick={() => setModal(true)}>
              Open modal
            </Button>
            <Button variant="destructive" onClick={() => setConfirm(true)}>
              Delete something
            </Button>
            <Tip content="A short explanation.">
              <Button variant="ghost">Hover for a tip</Button>
            </Tip>
          </div>
          <Modal
            open={modal}
            onOpenChange={setModal}
            title="A modal"
            description="One sentence about what this is for."
            footer={<Button onClick={() => setModal(false)}>Done</Button>}
          >
            <TextField label="A field" />
          </Modal>
          <ConfirmModal
            open={confirm}
            onOpenChange={setConfirm}
            title="Delete this?"
            description="It cannot be brought back."
            confirmLabel="Delete"
            destructive
            onConfirm={() => {
              setConfirm(false);
              toast.success("Deleted.");
            }}
          />
        </Demo>
      </Section>

      <Section id="forms" title="Forms">
        <Demo name="MethodSwitch">
          <div className="max-w-md">
            <MethodSwitch
              methods={["password", "wallet", "code", "external"]}
              value={method}
              onChange={setMethod}
            />
          </div>
        </Demo>
        <Demo name="ProviderButtons">
          <div className="max-w-md">
            <ProviderButtons providers={["google", "github"]} intent="login" />
          </div>
        </Demo>
        <Demo name="WalletConnect">
          <div className="max-w-md">
            <WalletConnect>
              {({ address }) => <p className="text-muted-foreground">Connected: {address}</p>}
            </WalletConnect>
          </div>
        </Demo>
        <Demo name="CodeField, SendAgain">
          <div className="grid max-w-md gap-2">
            <CodeField value={code} onChange={(e) => setCode(e.target.value)} />
            <SendAgain onSend={() => Promise.resolve()} />
          </div>
        </Demo>
        <Demo name="Field, TextField, TextareaField, PasswordInput, FormActions">
          <form className="grid max-w-md gap-4" onSubmit={(e) => e.preventDefault()}>
            <TextField label="Name" hint="As it appears to other people." />
            <TextField label="Email" error="Enter a valid email address." defaultValue="ana@" />
            <TextareaField label="Bio" hint="Optional." />
            <Field label="Password" hint="At least 12 characters.">
              {(c) => <PasswordInput {...c} />}
            </Field>
            <Field label="Choice">
              {(c) => (
                <Select
                  items={{ one: "One", two: "Two" }}
                  value={select}
                  onValueChange={(v) => setSelect(String(v))}
                >
                  <SelectTrigger id={c.id} className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="one">One</SelectItem>
                    <SelectItem value="two">Two</SelectItem>
                  </SelectContent>
                </Select>
              )}
            </Field>
            <div className="flex items-center gap-2">
              <Checkbox id="demo-check" checked={checked} onCheckedChange={setChecked} />
              <Label htmlFor="demo-check">A checkbox</Label>
            </div>
            <div className="flex items-center gap-2">
              <Switch id="demo-switch" />
              <Label htmlFor="demo-switch">A switch</Label>
            </div>
            <Input disabled placeholder="Disabled input" />
            <FormActions>
              <Button variant="outline" type="button">
                Cancel
              </Button>
              <Button type="submit">Save</Button>
            </FormActions>
          </form>
        </Demo>
      </Section>

      <Section id="content" title="Content">
        <Demo name="Markdown">
          <Markdown>{SAMPLE_MD}</Markdown>
        </Demo>
      </Section>

      <Section id="primitives" title="Primitives">
        <Demo name="Button">
          <div className="flex flex-wrap gap-2">
            <Button>Default</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="outline">Outline</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="destructive">Destructive</Button>
            <Button variant="link">Link</Button>
            <Button size="sm">Small</Button>
            <Button disabled>Disabled</Button>
          </div>
        </Demo>
        <Demo name="Badge">
          <div className="flex flex-wrap gap-2">
            <Badge>Default</Badge>
            <Badge variant="secondary">Secondary</Badge>
            <Badge variant="outline">Outline</Badge>
            <Badge variant="destructive">Destructive</Badge>
          </div>
        </Demo>
        <Demo name="Card">
          <Card className="max-w-sm">
            <CardHeader>
              <CardTitle>Card title</CardTitle>
              <CardDescription>One sentence under the title.</CardDescription>
            </CardHeader>
            <CardContent>Card content.</CardContent>
          </Card>
        </Demo>
        <Demo name="Tabs">
          <Tabs defaultValue="a">
            <TabsList>
              <TabsTrigger value="a">First</TabsTrigger>
              <TabsTrigger value="b">Second</TabsTrigger>
            </TabsList>
            <TabsContent value="a" className="pt-2">
              First panel.
            </TabsContent>
            <TabsContent value="b" className="pt-2">
              Second panel.
            </TabsContent>
          </Tabs>
        </Demo>
      </Section>
    </div>
  );
}

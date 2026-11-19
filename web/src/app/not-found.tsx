import Link from "next/link";

import { Empty } from "@/components/Empty/Empty";
import { Button } from "@/components/ui/button";

export default function NotFound() {
  return (
    <div className="flex min-h-dvh items-center justify-center">
      <Empty
        title="Page not found"
        description="The link is wrong, or the page has moved."
        action={
          <Button nativeButton={false} render={<Link href="/" />}>
            Go home
          </Button>
        }
      />
    </div>
  );
}

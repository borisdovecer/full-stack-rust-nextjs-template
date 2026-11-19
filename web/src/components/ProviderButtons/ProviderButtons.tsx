import { Button } from "@/components/ui/button";
import { PROVIDER_LABEL } from "@/lib/labels";

export type OauthIntent = "login" | "register" | "link";

/**
 * "Continue with Google / GitHub". Plain links: the API sends the browser to the provider and
 * back to the page, which reads `?error=` when it failed.
 */
export function ProviderButtons({
  providers,
  intent,
}: {
  providers: string[];
  intent: OauthIntent;
}) {
  return (
    <div className="grid gap-2">
      {providers.map((p) => {
        const label = `Continue with ${PROVIDER_LABEL[p] ?? p}`;
        return (
          <Button
            key={p}
            variant="outline"
            className="w-full"
            nativeButton={false}
            render={<a href={`/api/auth/oauth/${p}/start?intent=${intent}`} aria-label={label} />}
          >
            {label}
          </Button>
        );
      })}
    </div>
  );
}

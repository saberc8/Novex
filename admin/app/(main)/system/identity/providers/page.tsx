"use client";

import { RefreshCw, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { listIdentityProviders } from "@/api/system/identity";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { IdentityProviderResp } from "@/types/system-identity";

export default function IdentityProvidersPage() {
  const [providers, setProviders] = useState<IdentityProviderResp[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);

  const loadProviders = useCallback(async () => {
    setLoading(true);
    try {
      const result = await listIdentityProviders({ page: 1, size: 20 });
      setProviders(result.list);
      setTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "身份源加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders]);

  return (
    <PermissionGate permissions={["system:identityProvider:list"]}>
      <div className="mx-auto grid w-full max-w-7xl gap-4">
        <section className="flex flex-col gap-3 rounded-lg border bg-background p-4 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <ShieldCheck className="size-4 text-primary" />
              <h1 className="truncate text-base font-semibold">身份源</h1>
            </div>
            <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
              <span>{total} 个 Provider</span>
              <code className="rounded border bg-muted px-1.5 py-0.5">system:identityProvider:list</code>
            </div>
          </div>
          <Button variant="outline" onClick={() => void loadProviders()} disabled={loading}>
            <RefreshCw className={loading ? "size-4 animate-spin" : "size-4"} />
            刷新
          </Button>
        </section>

        <section className="grid gap-3">
          {providers.map((provider) => (
            <article key={provider.id} className="grid gap-3 rounded-lg border bg-background p-4">
              <div className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
                <div className="min-w-0">
                  <div className="truncate font-medium">{provider.name}</div>
                  <div className="mt-1 flex flex-wrap gap-2 text-xs text-muted-foreground">
                    <span>{provider.code}</span>
                    <span>{provider.maskedSecretRef || "no-secret"}</span>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Badge variant="outline">{provider.providerType}</Badge>
                  <Badge variant="secondary">{provider.status === 1 ? "启用" : "停用"}</Badge>
                </div>
              </div>
              <div className="flex flex-wrap gap-1.5">
                {policyBadges(provider).map((badge) => (
                  <Badge key={badge} variant="outline">
                    {badge}
                  </Badge>
                ))}
              </div>
            </article>
          ))}
          {!providers.length ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
              暂无身份源
            </div>
          ) : null}
        </section>
      </div>
    </PermissionGate>
  );
}

function policyBadges(provider: IdentityProviderResp) {
  const policy = provider.tenantPolicy;
  const badges: string[] = [];
  const boundary = policy.credentialBoundary;
  if (typeof boundary === "string") {
    badges.push(boundary);
  }
  const scopes = policy.defaultScopes;
  if (Array.isArray(scopes)) {
    for (const scope of scopes) {
      if (typeof scope === "string") {
        badges.push(scope);
      }
    }
  }
  return badges;
}

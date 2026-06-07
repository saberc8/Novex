"use client";

import { Lock, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { listIdentityPolicies } from "@/api/system/identity";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { IdentityPolicyResp } from "@/types/system-identity";

export default function IdentityPoliciesPage() {
  const [policies, setPolicies] = useState<IdentityPolicyResp[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);

  const loadPolicies = useCallback(async () => {
    setLoading(true);
    try {
      const result = await listIdentityPolicies({ page: 1, size: 20 });
      setPolicies(result.list);
      setTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "准入策略加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadPolicies();
  }, [loadPolicies]);

  return (
    <PermissionGate permissions={["system:identityPolicy:list"]}>
      <div className="mx-auto grid w-full max-w-7xl gap-4">
        <section className="flex flex-col gap-3 rounded-lg border bg-background p-4 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Lock className="size-4 text-primary" />
              <h1 className="truncate text-base font-semibold">准入策略</h1>
            </div>
            <div className="mt-1 text-xs text-muted-foreground">{total} 条策略</div>
          </div>
          <Button variant="outline" onClick={() => void loadPolicies()} disabled={loading}>
            <RefreshCw className={loading ? "size-4 animate-spin" : "size-4"} />
            刷新
          </Button>
        </section>
        <section className="grid gap-2">
          {policies.map((policy) => (
            <article key={policy.providerId} className="rounded-lg border bg-background p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="min-w-0">
                  <div className="truncate font-medium">{policy.providerName}</div>
                  <div className="mt-1 text-xs text-muted-foreground">{policy.providerCode}</div>
                </div>
                <Badge variant="outline">{policy.providerType}</Badge>
              </div>
              <pre className="mt-3 max-h-40 overflow-auto rounded-md bg-muted p-3 text-xs leading-5">
                {JSON.stringify(policy.tenantPolicy, null, 2)}
              </pre>
            </article>
          ))}
          {!policies.length ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
              暂无准入策略
            </div>
          ) : null}
        </section>
      </div>
    </PermissionGate>
  );
}

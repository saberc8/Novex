"use client";

import { RefreshCw, Users } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { listExternalAccounts } from "@/api/system/identity";
import { PermissionGate } from "@/components/permission/permission-gate";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { ExternalAccountResp } from "@/types/system-identity";

export default function IdentityAccountsPage() {
  const [accounts, setAccounts] = useState<ExternalAccountResp[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);

  const loadAccounts = useCallback(async () => {
    setLoading(true);
    try {
      const result = await listExternalAccounts({ page: 1, size: 20 });
      setAccounts(result.list);
      setTotal(result.total);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "外部账号加载失败");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadAccounts();
  }, [loadAccounts]);

  return (
    <PermissionGate permissions={["system:externalAccount:list"]}>
      <div className="mx-auto grid w-full max-w-7xl gap-4">
        <section className="flex flex-col gap-3 rounded-lg border bg-background p-4 md:flex-row md:items-center md:justify-between">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Users className="size-4 text-primary" />
              <h1 className="truncate text-base font-semibold">外部账号</h1>
            </div>
            <div className="mt-1 text-xs text-muted-foreground">{total} 个绑定</div>
          </div>
          <Button variant="outline" onClick={() => void loadAccounts()} disabled={loading}>
            <RefreshCw className={loading ? "size-4 animate-spin" : "size-4"} />
            刷新
          </Button>
        </section>
        <section className="grid gap-2">
          {accounts.map((account) => (
            <article key={account.id} className="rounded-lg border bg-background p-4">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="min-w-0">
                  <div className="truncate font-medium">{account.displayName || account.externalSubject}</div>
                  <div className="mt-1 text-xs text-muted-foreground">{account.externalSubject}</div>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Badge variant="outline">{account.providerCode}</Badge>
                  <Badge variant="secondary">User #{account.userId}</Badge>
                </div>
              </div>
            </article>
          ))}
          {!accounts.length ? (
            <div className="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">
              暂无外部账号绑定
            </div>
          ) : null}
        </section>
      </div>
    </PermissionGate>
  );
}

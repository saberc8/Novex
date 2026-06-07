"use client";

import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, Clock3, KeyRound, Link2, ShieldCheck } from "lucide-react";
import { resolvePublicShare } from "@/api/integration";
import type { PublicShareResp } from "@/types/integration";

export function PublicShareClient({ token }: { token: string }) {
  const [share, setShare] = useState<PublicShareResp | null>(null);
  const [status, setStatus] = useState<"loading" | "accepted" | "error">("loading");

  useEffect(() => {
    let mounted = true;

    resolvePublicShare(token)
      .then((response) => {
        if (!mounted) {
          return;
        }
        setShare(response);
        setStatus(response.accepted ? "accepted" : "error");
      })
      .catch(() => {
        if (mounted) {
          setStatus("error");
        }
      });

    return () => {
      mounted = false;
    };
  }, [token]);

  const auth = share?.auth;
  const limits = useMemo(() => {
    if (!auth) {
      return "-";
    }
    return `${auth.qpsLimit} QPS / ${auth.quotaLimit} quota`;
  }, [auth]);

  if (status === "error") {
    return (
      <main className="min-h-screen bg-slate-100 text-slate-950">
        <div className="mx-auto flex min-h-screen max-w-5xl items-center justify-center p-4">
          <section className="w-full max-w-md rounded-lg border border-rose-200 bg-white p-5 shadow-sm">
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-rose-600 text-white">
                <AlertTriangle aria-hidden="true" className="h-5 w-5" />
              </div>
              <div className="min-w-0">
                <h1 className="text-lg font-semibold text-slate-950">Share link unavailable</h1>
                <div className="mt-1 truncate font-mono text-xs text-slate-500">Token hidden</div>
              </div>
            </div>
          </section>
        </div>
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-slate-100 text-slate-950">
      <div className="mx-auto grid min-h-screen max-w-[1280px] grid-cols-1 lg:grid-cols-[minmax(0,1fr)_340px]">
        <section className="min-w-0 p-4 lg:p-6">
          <header className="rounded-lg border border-slate-200 bg-white p-5 shadow-sm">
            <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
              <div className="min-w-0">
                <div className="flex items-center gap-2 text-sm font-medium text-teal-700">
                  <Link2 aria-hidden="true" className="h-4 w-4" />
                  Public Link
                </div>
                <h1 className="mt-2 text-2xl font-semibold tracking-normal text-slate-950">Novex Share</h1>
                <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-600">
                  {status === "loading" ? "Resolving published access..." : auth?.name}
                </p>
              </div>
              <StatusBadge status={status} />
            </div>
          </header>

          <section className="mt-4 rounded-lg border border-slate-200 bg-white p-5 shadow-sm">
            <div className="grid gap-4 md:grid-cols-2">
              <DetailCard icon={<ShieldCheck aria-hidden="true" className="h-4 w-4" />} label="App" value={auth?.appId ?? "-"} />
              <DetailCard icon={<Link2 aria-hidden="true" className="h-4 w-4" />} label="Target" value={share?.targetPath ?? "-"} />
              <DetailCard icon={<KeyRound aria-hidden="true" className="h-4 w-4" />} label="Credential" value={auth?.maskedCredential ?? "-"} mono />
              <DetailCard icon={<Clock3 aria-hidden="true" className="h-4 w-4" />} label="Expires" value={auth?.expiresAt ?? "No expiry"} />
            </div>
          </section>
        </section>

        <aside className="border-t border-slate-200 bg-white p-4 lg:border-l lg:border-t-0 lg:p-5">
          <section className="rounded-lg border border-slate-200 p-4">
            <div className="text-xs font-semibold uppercase tracking-wide text-slate-500">Limits</div>
            <div className="mt-2 text-sm font-semibold text-slate-950">{limits}</div>
          </section>

          <section className="mt-4 rounded-lg border border-slate-200 p-4">
            <div className="text-xs font-semibold uppercase tracking-wide text-slate-500">Scope</div>
            <div className="mt-3 flex flex-wrap gap-2">
              {(auth?.permissionScope ?? []).map((scope) => (
                <span className="rounded-md bg-slate-100 px-2 py-1 font-mono text-xs text-slate-700" key={scope}>
                  {scope}
                </span>
              ))}
              {!auth?.permissionScope.length ? <span className="text-sm text-slate-500">-</span> : null}
            </div>
          </section>

          <section className="mt-4 rounded-lg border border-slate-200 p-4">
            <div className="text-xs font-semibold uppercase tracking-wide text-slate-500">Tenant</div>
            <div className="mt-2 text-sm font-semibold text-slate-950">{auth?.tenantId ?? "-"}</div>
          </section>
        </aside>
      </div>
    </main>
  );
}

function StatusBadge({ status }: { status: "loading" | "accepted" | "error" }) {
  if (status === "loading") {
    return (
      <span className="inline-flex h-9 items-center rounded-md bg-slate-100 px-3 text-sm font-medium text-slate-700">
        Loading
      </span>
    );
  }

  return (
    <span className="inline-flex h-9 items-center gap-2 rounded-md bg-teal-50 px-3 text-sm font-semibold text-teal-800 ring-1 ring-teal-100">
      <CheckCircle2 aria-hidden="true" className="h-4 w-4" />
      Accepted
    </span>
  );
}

function DetailCard({
  icon,
  label,
  value,
  mono
}: {
  icon: React.ReactNode;
  label: string;
  value: string | number;
  mono?: boolean;
}) {
  return (
    <article className="min-w-0 rounded-lg border border-slate-200 p-4">
      <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-slate-500">
        {icon}
        {label}
      </div>
      <div className={["mt-2 min-w-0 break-words text-sm font-semibold text-slate-950", mono ? "font-mono" : ""].join(" ")}>
        {value}
      </div>
    </article>
  );
}

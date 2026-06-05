import { ChevronRight, LogOut } from "lucide-react";
import { trainingNavItems } from "@/lib/navigation";

export function TrainingShell({ children }: { children: React.ReactNode }) {
  return (
    <main className="min-h-screen bg-slate-100 text-slate-950">
      <div className="mx-auto grid min-h-screen max-w-[1440px] grid-cols-1 lg:grid-cols-[260px_1fr]">
        <aside className="border-b border-slate-200 bg-white px-4 py-4 lg:border-b-0 lg:border-r lg:px-5 lg:py-6">
          <div className="flex items-center justify-between gap-3 lg:block">
            <div>
              <div className="text-lg font-semibold tracking-normal text-slate-950">Novex</div>
              <div className="mt-1 text-sm text-slate-500">AI 员工培训</div>
            </div>
            <button
              aria-label="退出"
              className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-slate-200 text-slate-500 hover:bg-slate-50 lg:hidden"
              type="button"
            >
              <LogOut aria-hidden="true" className="h-4 w-4" />
            </button>
          </div>

          <nav aria-label="培训导航" className="mt-5 grid grid-cols-5 gap-2 lg:grid-cols-1">
            {trainingNavItems.map((item, index) => {
              const Icon = item.icon;
              const active = index === 0;

              return (
                <a
                  className={`group flex min-h-16 items-center justify-center rounded-lg border px-2 py-2 text-center lg:justify-between lg:text-left ${
                    active
                      ? "border-teal-200 bg-teal-50 text-teal-950"
                      : "border-transparent text-slate-600 hover:border-slate-200 hover:bg-slate-50"
                  }`}
                  href={item.href}
                  key={item.href}
                >
                  <span className="flex min-w-0 flex-col items-center gap-1 lg:flex-row lg:gap-3">
                    <Icon
                      aria-hidden="true"
                      className={`h-5 w-5 shrink-0 ${active ? "text-teal-700" : "text-slate-400"}`}
                    />
                    <span className="min-w-0">
                      <span className="block text-xs font-semibold lg:text-sm">{item.label}</span>
                      <span className="hidden text-xs text-slate-500 lg:block">
                        {item.description}
                      </span>
                    </span>
                  </span>
                  <ChevronRight
                    aria-hidden="true"
                    className="hidden h-4 w-4 text-slate-300 group-hover:text-slate-500 lg:block"
                  />
                </a>
              );
            })}
          </nav>
        </aside>

        {children}
      </div>
    </main>
  );
}

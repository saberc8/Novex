import Link from "next/link";
import { LayoutGrid, Settings, ShieldCheck, SlidersHorizontal } from "lucide-react";
import { appRouteLinks } from "@/page-routes";

const settingsGroups = [
  {
    icon: SlidersHorizontal,
    title: "模型路由",
    description: "查看当前聊天和知识问答使用的运行时模型配置。"
  },
  {
    icon: ShieldCheck,
    title: "访问控制",
    description: "确认当前模板的登录、权限范围和分享入口设置。"
  }
];

export default function Page() {
  return (
    <main className="min-h-screen bg-[#eef0fb] text-neutral-950">
      <header className="flex h-[88px] items-center justify-between gap-5 px-6">
        <div className="flex min-w-0 items-center gap-4">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-black text-white">
            <LayoutGrid aria-hidden="true" className="h-6 w-6" />
          </div>
          <h1 className="truncate text-3xl font-medium tracking-normal">设置</h1>
        </div>
        <nav aria-label="页面导航" className="hidden min-w-0 flex-1 items-center justify-center gap-1 lg:flex">
          <div className="inline-flex rounded-full bg-slate-100 p-1">
            {appRouteLinks.map((item) => {
              const isActive = item.key === "settings";

              return (
                <Link
                  aria-current={isActive ? "page" : undefined}
                  className={[
                    "rounded-full px-3 py-2 text-sm font-medium transition-colors",
                    isActive ? "bg-white text-neutral-950 shadow-sm" : "text-neutral-600 hover:text-neutral-950"
                  ].join(" ")}
                  href={item.href}
                  key={item.key}
                >
                  {item.label}
                </Link>
              );
            })}
          </div>
        </nav>
      </header>

      <section className="mx-auto max-w-5xl px-6 pb-12 pt-4">
        <div className="rounded-lg bg-white p-6">
          <div className="flex items-center gap-3">
            <Settings aria-hidden="true" className="h-6 w-6 text-neutral-700" />
            <div>
              <h2 className="text-xl font-semibold">NotebookLM 配置</h2>
              <p className="mt-1 text-sm text-neutral-500">知识工作区的页面、权限和模型运行配置。</p>
            </div>
          </div>
          <div className="mt-6 grid gap-4 md:grid-cols-2">
            {settingsGroups.map((group) => (
              <article className="rounded-lg border border-slate-200 p-5" key={group.title}>
                <group.icon aria-hidden="true" className="h-5 w-5 text-neutral-700" />
                <h3 className="mt-4 text-base font-semibold">{group.title}</h3>
                <p className="mt-2 text-sm leading-6 text-neutral-500">{group.description}</p>
              </article>
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}

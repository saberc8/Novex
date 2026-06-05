import { Bell, BookOpenCheck, ClipboardCheck, History, MessageSquareText } from "lucide-react";
import type { LucideIcon } from "lucide-react";

export interface TrainingNavItem {
  href: string;
  label: string;
  description: string;
  icon: LucideIcon;
}

export const trainingNavItems: TrainingNavItem[] = [
  {
    href: "/",
    label: "学习",
    description: "任务、进度、资料入口",
    icon: BookOpenCheck
  },
  {
    href: "/ask",
    label: "问答",
    description: "基于培训资料提问",
    icon: MessageSquareText
  },
  {
    href: "/quiz",
    label: "测验",
    description: "自动出题和错题回顾",
    icon: ClipboardCheck
  },
  {
    href: "/records",
    label: "记录",
    description: "学习和答题记录",
    icon: History
  },
  {
    href: "/notifications",
    label: "通知",
    description: "飞书任务和提醒状态",
    icon: Bell
  }
];

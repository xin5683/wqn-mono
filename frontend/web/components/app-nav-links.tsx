'use client';

import { Link } from '@/i18n/navigation';
import { usePathname } from '@/i18n/navigation';
import {
  BookOpen,
  FolderOpen,
  Globe,
  BarChart3,
  Lightbulb,
} from 'lucide-react';
import { useTranslations } from 'next-intl';

import { cn } from '@/lib/utils';

// Shared by the desktop top nav (AppNavLinks) and the mobile bottom tab bar
// (MobileTabBar). Keep icon + colour per entry so both surfaces stay in sync.
export const APP_LINKS = [
  {
    href: '/subjects',
    labelKey: 'subjects' as const,
    icon: BookOpen,
    iconBg: 'bg-amber-500/10 dark:bg-amber-500/20',
    iconColor: 'text-amber-600 dark:text-amber-400',
  },
  {
    href: '/problem-sets',
    labelKey: 'problemSets' as const,
    icon: FolderOpen,
    iconBg: 'bg-blue-500/10 dark:bg-blue-500/20',
    iconColor: 'text-blue-600 dark:text-blue-400',
  },
  {
    href: '/discover',
    labelKey: 'discover' as const,
    icon: Globe,
    iconBg: 'bg-purple-500/10 dark:bg-purple-500/20',
    iconColor: 'text-purple-600 dark:text-purple-400',
  },
  {
    href: '/statistics',
    labelKey: 'statistics' as const,
    icon: BarChart3,
    iconBg: 'bg-green-500/10 dark:bg-green-500/20',
    iconColor: 'text-green-600 dark:text-green-400',
  },
  {
    href: '/insights',
    labelKey: 'insights' as const,
    icon: Lightbulb,
    iconBg: 'bg-orange-500/10 dark:bg-orange-500/20',
    iconColor: 'text-orange-600 dark:text-orange-400',
  },
];

export function isActivePath(pathname: string | null, href: string) {
  if (!pathname) return false;
  return pathname === href || pathname.startsWith(`${href}/`);
}

function NavLink({
  href,
  labelKey,
}: {
  href: string;
  labelKey: (typeof APP_LINKS)[number]['labelKey'];
}) {
  const t = useTranslations('Navigation');
  const pathname = usePathname();
  const active = isActivePath(pathname, href);

  return (
    <Link
      href={href}
      className={cn(
        'text-base text-muted-foreground hover:text-foreground transition-all duration-200 border-b-2 border-transparent pb-0.5 hover:border-amber-300/50 dark:hover:border-amber-700/50',
        active &&
          'text-foreground font-medium border-amber-500 dark:border-amber-400 hover:border-amber-500 dark:hover:border-amber-400'
      )}
      aria-current={active ? 'page' : undefined}
    >
      {t(labelKey)}
    </Link>
  );
}

// Desktop-only horizontal nav. On mobile, navigation lives in the bottom tab
// bar (components/mobile-tab-bar.tsx) — this surface is hidden below `md`.
export function AppNavLinks() {
  return (
    <div className="hidden items-center gap-6 md:flex">
      {APP_LINKS.map(l => (
        <NavLink key={l.href} href={l.href} labelKey={l.labelKey} />
      ))}
    </div>
  );
}

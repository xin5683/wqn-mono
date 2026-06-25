'use client';

import { Link, usePathname } from '@/i18n/navigation';
import { useTranslations } from 'next-intl';
import { cn } from '@/lib/utils';
import { APP_LINKS, isActivePath } from '@/components/app-nav-links';

// Persistent bottom tab bar for mobile (<md). Mirrors APP_LINKS so the 5 core
// entries are one tap away — App-grade reachability vs. the old hamburger menu.
// Desktop renders the top-nav AppNavLinks instead; both stay in sync via the
// shared APP_LINKS / isActivePath exports.
export function MobileTabBar() {
  const pathname = usePathname();
  const t = useTranslations('Navigation');

  return (
    <nav
      aria-label="Mobile navigation"
      className="fixed bottom-0 inset-x-0 z-50 md:hidden border-t border-amber-200/40 bg-background/95 backdrop-blur pb-[env(safe-area-inset-bottom)] dark:border-amber-800/30"
    >
      <ul className="grid h-14 grid-cols-5">
        {APP_LINKS.map(l => {
          const active = isActivePath(pathname, l.href);
          const Icon = l.icon;
          return (
            <li key={l.href}>
              <Link
                href={l.href}
                aria-current={active ? 'page' : undefined}
                className="flex h-full w-full flex-col items-center justify-center gap-0.5"
              >
                <span
                  className={cn(
                    'flex h-7 w-7 items-center justify-center rounded-lg',
                    active ? l.iconBg : 'bg-transparent'
                  )}
                >
                  <Icon
                    className={cn(
                      'h-5 w-5',
                      active ? l.iconColor : 'text-muted-foreground'
                    )}
                  />
                </span>
                <span
                  className={cn(
                    'hidden text-[10px] leading-none min-[400px]:block',
                    active
                      ? cn(l.iconColor, 'font-medium')
                      : 'text-muted-foreground'
                  )}
                >
                  {t(l.labelKey)}
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}

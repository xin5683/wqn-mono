// web/src/app/[locale]/(app)/layout.tsx
import { Navigation } from '@/components/navigation';
import { AnnouncementBanner } from '@/components/announcement-banner';
import { OnboardingProvider } from '@/components/onboarding/onboarding-provider';
import { TimezoneSync } from '@/components/timezone-sync';
import { getCurrentUser } from '@/lib/api/server';
import '@/app/globals.css';

interface LocaleAppLayoutProps {
  children: React.ReactNode;
}

export default async function LocaleAppLayout({
  children,
}: LocaleAppLayoutProps) {
  let showOnboarding = false;
  let currentTimezone: string | null = null;

  try {
    const user = await getCurrentUser();
    if (user?.profile) {
      const profile = user.profile;
      showOnboarding = profile?.onboarding_completed_at === null;
      currentTimezone = profile?.timezone ?? null;
    }
  } catch {
    // If fetching fails, don't show onboarding
  }

  return (
    <div className="min-h-screen bg-gradient-to-b from-amber-50/80 via-white to-rose-50/50 dark:from-stone-950 dark:via-stone-950 dark:to-stone-950">
      <TimezoneSync currentTimezone={currentTimezone} />
      <AnnouncementBanner />
      <Navigation showAppLinks={true} sticky={true} />
      <OnboardingProvider showOnboarding={showOnboarding}>
        <main id="main-content" className="page-container main-content">
          {children}
        </main>
      </OnboardingProvider>
    </div>
  );
}

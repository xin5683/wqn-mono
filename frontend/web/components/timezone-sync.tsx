'use client';

import { useEffect } from 'react';
import { validatePayload } from '@/lib/validation/payload';
import { UpdateUserProfileDto } from '@/lib/validation/schemas';
import { clientApi } from '@/lib/api/client';

/**
 * Invisible client component that auto-detects the user's browser timezone
 * on every authenticated page load and syncs it to their profile if it differs.
 */
export function TimezoneSync({
  currentTimezone,
}: {
  currentTimezone: string | null;
}) {
  useEffect(() => {
    const detected = Intl.DateTimeFormat().resolvedOptions().timeZone;
    if (!detected || detected === currentTimezone) return;

    try {
      const payload = validatePayload(
        { timezone: detected },
        UpdateUserProfileDto,
        'update profile'
      );

      // Fire-and-forget PATCH — no need to block UI
      clientApi('/api/profile', {
        method: 'PATCH',
        body: payload,
      }).catch(() => {
        // Silently ignore — timezone sync is best-effort
      });
    } catch {
      // Silently ignore — timezone sync is best-effort
    }
  }, [currentTimezone]);

  return null;
}

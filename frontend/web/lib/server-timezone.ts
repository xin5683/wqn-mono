import { getCurrentUser } from '@/lib/api/server';
import { DEFAULT_TIMEZONE, isValidTimezone } from '@/lib/utils/timezone';

/**
 * Look up the timezone for a user from the current authenticated profile.
 * Falls back to UTC on any error or missing value.
 */
export async function getUserTimezone(userId: string): Promise<string> {
  try {
    const user = await getCurrentUser();
    const timezone = user?.id === userId ? user.profile?.timezone : null;
    if (!timezone) return DEFAULT_TIMEZONE;
    return isValidTimezone(timezone) ? timezone : DEFAULT_TIMEZONE;
  } catch {
    return DEFAULT_TIMEZONE;
  }
}

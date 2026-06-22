import { notFound } from 'next/navigation';
import {
  getAdminUserProfile,
  getUserContentStatistics,
  getUserActivity,
  getUserStorageUsage,
} from '@/lib/api/user-management';
import { getQuotaUsage } from '@/lib/api/usage-quota';
import { getAllContentLimits } from '@/lib/api/content-limits';
import { UserDetailClient } from '@/components/admin/users/user-detail-client';

export default async function UserDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;

  const [
    profile,
    contentStats,
    quotaUsage,
    activity,
    storageUsage,
    contentLimits,
  ] = await Promise.all([
    getAdminUserProfile(id),
    getUserContentStatistics(id),
    getQuotaUsage(id).catch(() => null),
    getUserActivity(id, 10),
    getUserStorageUsage(id),
    getAllContentLimits(id),
  ]);

  if (!profile) {
    notFound();
  }

  return (
    <UserDetailClient
      profile={profile}
      contentStats={contentStats}
      quotaUsage={quotaUsage}
      activity={activity}
      storageUsage={storageUsage}
      contentLimits={contentLimits}
    />
  );
}

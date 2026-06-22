import {
  getAdminDashboardStatistics,
  getRecentActivity,
} from '@/lib/api/user-management';
import { DashboardClient } from '@/components/admin/dashboard/dashboard-client';

export default async function AdminDashboardPage() {
  const [statistics, recentActivity] = await Promise.all([
    getAdminDashboardStatistics(),
    getRecentActivity(5),
  ]);
  const { userStats, contentStats, storageStats } = statistics;

  return (
    <DashboardClient
      userStats={userStats}
      contentStats={contentStats}
      storageStats={storageStats}
      recentActivity={recentActivity}
    />
  );
}

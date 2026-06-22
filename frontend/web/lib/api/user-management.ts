import { getCurrentUser, serverApi } from '@/lib/api/server';
import type {
  AdminSettingsType,
  UserActivityLogType,
  UserProfileType,
  UserStatisticsType,
} from '@/lib/validation/schemas';

interface AdminStatisticsPayload {
  statistics?: Partial<UserStatisticsType>;
  contentStats?: {
    subjects?: number;
    problems?: number;
    problem_sets?: number;
    attempts?: number;
  };
  storageStats?: {
    total_bytes?: number;
    file_count?: number;
  };
}

interface ContentStatistics {
  total_subjects: number;
  total_problems: number;
  total_problem_sets: number;
  total_attempts: number;
}

interface StorageUsage {
  totalBytes: number;
  fileCount: number;
}

interface UserContentStatistics {
  subjects: number;
  problems: number;
  problem_sets: number;
  attempts: number;
}

interface AdminDashboardStatistics {
  userStats: UserStatisticsType;
  contentStats: ContentStatistics;
  storageStats: StorageUsage;
}

interface AdminActivityResponse {
  activities?: UserActivityLogType[];
  total_count?: number;
}

interface AdminUsersResponse {
  users?: UserProfileType[];
  total_count?: number;
  page?: number;
  limit?: number;
}

interface AdminSettingsResponse {
  settings?: AdminSettingsType[];
}

interface AnnouncementResponse {
  enabled?: boolean;
  message?: string;
  type?: 'info' | 'warning' | 'success';
}

const emptyUserStatistics: UserStatisticsType = {
  total_users: 0,
  active_users: 0,
  admin_users: 0,
  new_users_today: 0,
  new_users_this_week: 0,
};

const emptyContentStatistics: ContentStatistics = {
  total_subjects: 0,
  total_problems: 0,
  total_problem_sets: 0,
  total_attempts: 0,
};

const emptyStorageUsage: StorageUsage = {
  totalBytes: 0,
  fileCount: 0,
};

export async function getUserProfile(
  _userId: string
): Promise<UserProfileType | null> {
  void _userId;
  const user = await getCurrentUser();
  return (user?.profile as UserProfileType | null) ?? null;
}

export async function getAdminUserProfile(
  userId: string
): Promise<UserProfileType | null> {
  return serverApi<UserProfileType>(
    `/api/admin/users/${encodeURIComponent(userId)}`
  ).catch(() => null);
}

export async function isCurrentUserSuperAdmin(): Promise<boolean> {
  const user = await getCurrentUser();
  return user?.role === 'super_admin';
}

export async function getAdminDashboardStatistics(): Promise<AdminDashboardStatistics> {
  const data = await serverApi<AdminStatisticsPayload>(
    '/api/admin/statistics'
  ).catch(() => null);

  return {
    userStats: {
      total_users:
        data?.statistics?.total_users ?? emptyUserStatistics.total_users,
      active_users:
        data?.statistics?.active_users ?? emptyUserStatistics.active_users,
      admin_users:
        data?.statistics?.admin_users ?? emptyUserStatistics.admin_users,
      new_users_today:
        data?.statistics?.new_users_today ??
        emptyUserStatistics.new_users_today,
      new_users_this_week:
        data?.statistics?.new_users_this_week ??
        emptyUserStatistics.new_users_this_week,
    },
    contentStats: {
      total_subjects:
        data?.contentStats?.subjects ?? emptyContentStatistics.total_subjects,
      total_problems:
        data?.contentStats?.problems ?? emptyContentStatistics.total_problems,
      total_problem_sets:
        data?.contentStats?.problem_sets ??
        emptyContentStatistics.total_problem_sets,
      total_attempts:
        data?.contentStats?.attempts ?? emptyContentStatistics.total_attempts,
    },
    storageStats: {
      totalBytes:
        data?.storageStats?.total_bytes ?? emptyStorageUsage.totalBytes,
      fileCount: data?.storageStats?.file_count ?? emptyStorageUsage.fileCount,
    },
  };
}

export async function getRecentActivity(
  limit = 10
): Promise<UserActivityLogType[]> {
  const data = await serverApi<AdminActivityResponse>(
    `/api/admin/activity?limit=${limit}`
  ).catch(() => ({ activities: [] }));
  return data.activities ?? [];
}

export async function getFilteredActivity(options: {
  user_id?: string;
  action?: string;
  limit?: number;
  offset?: number;
}): Promise<{ activities: UserActivityLogType[]; total_count: number }> {
  const limit = options.limit ?? 20;
  const page = Math.floor((options.offset ?? 0) / limit) + 1;
  const params = new URLSearchParams({
    limit: String(limit),
    page: String(page),
  });
  if (options.user_id) params.set('user_id', options.user_id);
  if (options.action) params.set('action', options.action);
  const data = await serverApi<AdminActivityResponse>(
    `/api/admin/activity?${params}`
  ).catch(() => ({ activities: [], total_count: 0 }));
  return {
    activities: data.activities ?? [],
    total_count: data.total_count ?? 0,
  };
}

export async function getAllUsersWithCount(
  limit = 20,
  offset = 0
): Promise<{ users: UserProfileType[]; total_count: number }> {
  const page = Math.floor(offset / limit) + 1;
  const data = await serverApi<AdminUsersResponse>(
    `/api/admin/users?limit=${limit}&page=${page}`
  ).catch(() => ({ users: [], total_count: 0 }));
  return {
    users: data.users ?? [],
    total_count: data.total_count ?? 0,
  };
}

export async function getAdminSettings(): Promise<AdminSettingsType[]> {
  const data = await serverApi<AdminSettingsResponse>(
    '/api/admin/settings'
  ).catch(() => ({
    settings: [],
  }));
  return data.settings ?? [];
}

export async function getAnnouncement(): Promise<{
  enabled: boolean;
  message: string;
  type: 'info' | 'warning' | 'success';
}> {
  const data = await serverApi<AnnouncementResponse>('/api/announcement').catch(
    () => null
  );
  const type =
    data?.type === 'warning' || data?.type === 'success' ? data.type : 'info';
  return {
    enabled: data?.enabled ?? false,
    message: data?.message ?? '',
    type,
  };
}

export async function getUserContentStatistics(
  userId: string
): Promise<UserContentStatistics> {
  return serverApi<UserContentStatistics>(
    `/api/admin/users/${encodeURIComponent(userId)}/content-statistics`
  ).catch(() => ({
    subjects: 0,
    problems: 0,
    problem_sets: 0,
    attempts: 0,
  }));
}

export async function getUserActivity(
  userId: string,
  limit = 10
): Promise<UserActivityLogType[]> {
  const data = await serverApi<AdminActivityResponse>(
    `/api/admin/activity?user_id=${encodeURIComponent(userId)}&limit=${limit}`
  ).catch(() => ({ activities: [] }));
  return data.activities ?? [];
}

export async function getUserStorageUsage(
  userId: string
): Promise<StorageUsage> {
  return serverApi<StorageUsage>(
    `/api/admin/users/${encodeURIComponent(userId)}/storage-usage`
  ).catch(() => ({ ...emptyStorageUsage }));
}

import type { Metadata } from 'next';
import { getTranslations } from 'next-intl/server';
import StatisticsPageClient from './statistics-page-client';
import { serverApi } from '@/lib/api/server';
import type { StatisticsData } from '@/lib/types';

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations('Metadata');
  return {
    title: t('statisticsMetaTitle'),
    description: t('statisticsMetaDescription'),
  };
}

const emptyData: StatisticsData = {
  overview: {
    total_problems: 0,
    mastered_count: 0,
    needs_review_count: 0,
    wrong_count: 0,
    mastery_rate: 0,
  },
  streaks: { current_streak: 0, longest_streak: 0 },
  sessionStats: {
    total_sessions: 0,
    avg_duration_ms: 0,
    avg_problems_per_session: 0,
    total_review_time_ms: 0,
  },
  subjectBreakdown: [],
  weeklyProgress: [],
  activityHeatmap: [],
  recentActivity: [],
  timezone: 'UTC',
};

async function loadStatistics() {
  return serverApi<StatisticsData>('/api/statistics').catch(() => emptyData);
}

export default async function StatisticsPage() {
  const data = await loadStatistics();
  return <StatisticsPageClient data={data} />;
}

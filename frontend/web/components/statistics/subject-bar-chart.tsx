'use client';

import dynamic from 'next/dynamic';
import type { SubjectBarChartProps } from './subject-bar-chart.impl';

const LazySubjectBarChart = dynamic<SubjectBarChartProps>(
  () => import('./subject-bar-chart.impl').then(mod => mod.SubjectBarChart),
  {
    ssr: false,
    loading: () => (
      <div className="h-full w-full rounded-md bg-muted/40 animate-pulse" />
    ),
  }
);

export function SubjectBarChart(props: SubjectBarChartProps) {
  return <LazySubjectBarChart {...props} />;
}

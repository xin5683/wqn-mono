'use client';

import dynamic from 'next/dynamic';
import type { SubjectRadarChartProps } from './subject-radar-chart.impl';

const LazySubjectRadarChart = dynamic<SubjectRadarChartProps>(
  () => import('./subject-radar-chart.impl').then(mod => mod.SubjectRadarChart),
  {
    ssr: false,
    loading: () => (
      <div className="mx-auto h-full w-full max-w-[250px] rounded-full bg-muted/40 animate-pulse" />
    ),
  }
);

export function SubjectRadarChart(props: SubjectRadarChartProps) {
  return <LazySubjectRadarChart {...props} />;
}

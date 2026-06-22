'use client';

import dynamic from 'next/dynamic';
import type { ProgressLineChartProps } from './progress-line-chart.impl';

const LazyProgressLineChart = dynamic<ProgressLineChartProps>(
  () => import('./progress-line-chart.impl').then(mod => mod.ProgressLineChart),
  {
    ssr: false,
    loading: () => (
      <div className="h-full w-full rounded-md bg-muted/40 animate-pulse" />
    ),
  }
);

export function ProgressLineChart(props: ProgressLineChartProps) {
  return <LazyProgressLineChart {...props} />;
}

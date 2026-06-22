'use client';

import dynamic from 'next/dynamic';
import type { StatusDoughnutChartProps } from './status-doughnut-chart.impl';

const LazyStatusDoughnutChart = dynamic<StatusDoughnutChartProps>(
  () =>
    import('./status-doughnut-chart.impl').then(mod => mod.StatusDoughnutChart),
  {
    ssr: false,
    loading: () => (
      <div className="mx-auto h-44 w-44 rounded-full bg-muted/40 animate-pulse" />
    ),
  }
);

export function StatusDoughnutChart(props: StatusDoughnutChartProps) {
  return <LazyStatusDoughnutChart {...props} />;
}

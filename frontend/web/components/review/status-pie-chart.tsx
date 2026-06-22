'use client';

import dynamic from 'next/dynamic';
import type { StatusPieChartProps } from './status-pie-chart.impl';

const LazyStatusPieChart = dynamic<StatusPieChartProps>(
  () => import('./status-pie-chart.impl'),
  {
    ssr: false,
    loading: () => (
      <div className="h-[180px] w-[180px] rounded-full bg-muted/40 animate-pulse" />
    ),
  }
);

export default function StatusPieChart(props: StatusPieChartProps) {
  return <LazyStatusPieChart {...props} />;
}

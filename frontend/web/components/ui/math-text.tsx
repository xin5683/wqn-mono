'use client';

import dynamic from 'next/dynamic';
import { containsMath } from './math-text-utils';
import type { MathTextProps } from './math-text.impl';

const LazyMathText = dynamic<MathTextProps>(() => import('./math-text.impl'), {
  ssr: false,
  loading: () => <span className="inline-block h-4 w-16 bg-muted/40" />,
});

export { containsMath };

export default function MathText({ text, className }: MathTextProps) {
  if (!containsMath(text)) {
    return <span className={className}>{text}</span>;
  }

  return <LazyMathText text={text} className={className} />;
}

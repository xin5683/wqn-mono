'use client';

import React from 'react';
import dynamic from 'next/dynamic';
import { EditorSkeleton } from './editor-skeleton';
import type {
  RichTextEditorHandle,
  RichTextEditorProps,
} from './rich-text-editor.types';

const LazyRichTextEditor = dynamic(() => import('./rich-text-editor.impl'), {
  ssr: false,
  loading: () => <EditorSkeleton height="300px" />,
});

const RichTextEditor = React.forwardRef<
  RichTextEditorHandle,
  RichTextEditorProps
>(function RichTextEditor(props, ref) {
  return <LazyRichTextEditor {...props} forwardedRef={ref} />;
});

export { RichTextEditor };
export type { RichTextEditorHandle, RichTextEditorProps };

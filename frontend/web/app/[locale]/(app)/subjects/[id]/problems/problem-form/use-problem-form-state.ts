'use client';

import { useEffect, useMemo, useState } from 'react';
import type { ProblemType } from '@/lib/validation/schemas';
import type { Problem } from '@/lib/types';

type ProblemStatus = 'wrong' | 'needs_review' | 'mastered';

interface UseProblemFormStateOptions {
  problem: Problem | null;
  isEditMode: boolean;
  alwaysExpanded: boolean;
  initialShowImageScan: boolean;
}

export function useProblemFormState({
  problem,
  isEditMode,
  alwaysExpanded,
  initialShowImageScan,
}: UseProblemFormStateOptions) {
  const [editorKey, setEditorKey] = useState(0);
  const [isExpanded, setIsExpanded] = useState(isEditMode || alwaysExpanded);
  const [showImageScan, setShowImageScan] = useState(initialShowImageScan);
  const [title, setTitle] = useState(problem?.title || '');
  const [titleFocus, setTitleFocus] = useState(false);
  const [content, setContent] = useState(problem?.content || '');
  const [problemType, setProblemType] = useState<ProblemType>(
    problem?.problem_type || 'short'
  );
  const [status, setStatus] = useState<ProblemStatus>(
    problem?.status || 'needs_review'
  );
  const [autoMark, setAutoMark] = useState(problem?.auto_mark || false);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    if (isEditMode) return;
    switch (problemType) {
      case 'mcq':
        setAutoMark(true);
        break;
      case 'short':
      case 'extended':
        setAutoMark(false);
        break;
    }
  }, [problemType, isEditMode]);

  const autoMarkValue = useMemo(() => {
    switch (problemType) {
      case 'mcq':
      case 'short':
        return autoMark;
      case 'extended':
        return false;
      default:
        return false;
    }
  }, [problemType, autoMark]);

  function resetAfterCreate() {
    setTitle('');
    setContent('');
    setEditorKey(k => k + 1);
    setProblemType('short');
    setStatus('needs_review');
    setAutoMark(false);
    setIsExpanded(false);
  }

  return {
    editorKey,
    setEditorKey,
    isExpanded,
    setIsExpanded,
    showImageScan,
    setShowImageScan,
    title,
    setTitle,
    titleFocus,
    setTitleFocus,
    content,
    setContent,
    problemType,
    setProblemType,
    status,
    setStatus,
    autoMark,
    setAutoMark,
    autoMarkValue,
    isAutoMarkDisabled: problemType === 'extended',
    isSubmitting,
    setIsSubmitting,
    resetAfterCreate,
  };
}

export type ProblemFormState = ReturnType<typeof useProblemFormState>;

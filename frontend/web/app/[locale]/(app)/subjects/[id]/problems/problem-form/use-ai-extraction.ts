'use client';

import { RefObject, useCallback, useEffect, useState } from 'react';
import { clientApi } from '@/lib/api/client';
import { convertMathTextToTipTapHtml } from '@/lib/utils/math-to-tiptap';
import type { ProblemType } from '@/lib/validation/schemas';
import type { ExtractedProblemData } from '@/lib/types';
import type {
  ExtractionQuota,
  ImageAttachment,
} from '@/components/ui/image-scan-uploader';
import type { RichTextEditorHandle } from '@/components/editor';
import type { AnswerConfigState } from './use-answer-config';
import type { ProblemAssetsState } from './use-problem-assets';
import type { TagPickerState } from './use-tag-picker';

interface UseAiExtractionOptions {
  isEditMode: boolean;
  contentEditorRef: RefObject<RichTextEditorHandle | null>;
  setTitle: (title: string) => void;
  setProblemType: (problemType: ProblemType) => void;
  setContent: (content: string) => void;
  setShowImageScan: (show: boolean) => void;
  setIsExpanded: (expanded: boolean) => void;
  answerConfig: AnswerConfigState;
  assets: ProblemAssetsState;
  tagPicker: TagPickerState;
}

export function useAiExtraction({
  isEditMode,
  contentEditorRef,
  setTitle,
  setProblemType,
  setContent,
  setShowImageScan,
  setIsExpanded,
  answerConfig,
  assets,
  tagPicker,
}: UseAiExtractionOptions) {
  const [extractionQuota, setExtractionQuota] =
    useState<ExtractionQuota | null>(null);

  useEffect(() => {
    if (isEditMode) return;
    clientApi<ExtractionQuota>('/api/ai/extract-problem/quota')
      .then(setExtractionQuota)
      .catch(() => {});
  }, [isEditMode]);

  const handleExtractionComplete = useCallback(
    (data: ExtractedProblemData, imageAttachment?: ImageAttachment) => {
      setTitle(data.title);
      setProblemType(data.problem_type);

      const html = convertMathTextToTipTapHtml(data.content);
      contentEditorRef.current?.setContent(html);
      setContent(html);

      answerConfig.applyExtractedAnswerHint(
        data,
        assets.setSolutionHtml,
        convertMathTextToTipTapHtml
      );

      if (imageAttachment) {
        const roles: ('problem' | 'solution')[] = [];
        if (imageAttachment.saveAsProblemAsset) roles.push('problem');
        if (imageAttachment.saveAsSolutionAsset) roles.push('solution');
        assets.queueImageAttachment(imageAttachment.file, roles);
      }

      tagPicker.applySuggestedTags(data.suggested_tags);

      setShowImageScan(false);
      setIsExpanded(true);
    },
    [
      assets,
      answerConfig,
      contentEditorRef,
      setContent,
      setIsExpanded,
      setProblemType,
      setShowImageScan,
      setTitle,
      tagPicker,
    ]
  );

  return {
    extractionQuota,
    setExtractionQuota,
    handleExtractionComplete,
  };
}

export type AiExtractionState = ReturnType<typeof useAiExtraction>;

'use client';

import type { FormEvent } from 'react';
import { useMemo, useRef } from 'react';
import { useTranslations } from 'next-intl';
import { toast } from 'sonner';
import { VALIDATION_CONSTANTS } from '@/lib/constants';
import { useCreateProblem, useUpdateProblem } from './use-problems';
import { useUnsavedChanges } from '@/lib/hooks/useUnsavedChanges';
import type { RichTextEditorHandle } from '@/components/editor';
import { validatePayload } from '@/lib/validation/payload';
import { CreateProblemDto, UpdateProblemDto } from '@/lib/validation/schemas';
import type { ProblemFormProps } from '@/lib/types';
import { ProblemFormCollapsed } from './problem-form/problem-form-collapsed';
import { ProblemFormFull } from './problem-form/problem-form-full';
import { ProblemFormScan } from './problem-form/problem-form-scan';
import { useAiExtraction } from './problem-form/use-ai-extraction';
import { useAnswerConfig } from './problem-form/use-answer-config';
import { useProblemAssets } from './problem-form/use-problem-assets';
import { useProblemFormState } from './problem-form/use-problem-form-state';
import { useTagPicker } from './problem-form/use-tag-picker';

export default function ProblemForm({
  subjectId,
  availableTags = [],
  problem = null,
  onCancel = null,
  onProblemCreated = null,
  onProblemUpdated = null,
  alwaysExpanded = false,
  initialShowImageScan = false,
}: ProblemFormProps) {
  const t = useTranslations('Subjects');
  const isEditMode = !!problem;
  const createProblem = useCreateProblem(subjectId);
  const updateProblem = useUpdateProblem(subjectId);
  const contentEditorRef = useRef<RichTextEditorHandle>(null);
  const solutionEditorRef = useRef<RichTextEditorHandle>(null);

  const formState = useProblemFormState({
    problem,
    isEditMode,
    alwaysExpanded,
    initialShowImageScan,
  });
  const answerConfig = useAnswerConfig({
    problem,
    problemType: formState.problemType,
    isEditMode,
  });
  const tagPicker = useTagPicker({
    subjectId,
    availableTags,
    problem,
  });
  const assets = useProblemAssets({
    problem,
    isEditMode,
    isExpanded: formState.isExpanded,
    contentEditorRef,
    solutionEditorRef,
  });
  const aiExtraction = useAiExtraction({
    isEditMode,
    contentEditorRef,
    setTitle: formState.setTitle,
    setProblemType: formState.setProblemType,
    setContent: formState.setContent,
    setShowImageScan: formState.setShowImageScan,
    setIsExpanded: formState.setIsExpanded,
    answerConfig,
    assets,
    tagPicker,
  });

  const hasUnsavedData = useMemo(() => {
    if (!formState.isExpanded && !isEditMode && !alwaysExpanded) {
      return false;
    }

    return (
      formState.title.trim().length > 0 ||
      formState.content.length > 0 ||
      assets.problemAssets.length > 0 ||
      assets.solutionText.length > 0 ||
      assets.solutionAssets.length > 0 ||
      tagPicker.selectedTagIds.length > 0 ||
      tagPicker.pendingNewTags.some(
        name => !tagPicker.deselectedPendingTags.has(name)
      )
    );
  }, [
    formState.isExpanded,
    formState.title,
    formState.content,
    isEditMode,
    alwaysExpanded,
    assets.problemAssets,
    assets.solutionText,
    assets.solutionAssets,
    tagPicker.selectedTagIds,
    tagPicker.pendingNewTags,
    tagPicker.deselectedPendingTags,
  ]);

  useUnsavedChanges(hasUnsavedData);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();

    if (!formState.title.trim()) {
      toast.error(t('titleRequired'));
      return;
    }

    try {
      answerConfig.validateForSubmit(t);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : 'Invalid answer configuration'
      );
      return;
    }

    formState.setIsSubmitting(true);

    try {
      const problemAssetsPayload = assets.problemAssets.map(asset => ({
        path: asset.path,
      }));
      const solutionAssetsPayload = assets.solutionAssets.map(asset => ({
        path: asset.path,
      }));
      const sanitizedTitle = formState.title
        .trim()
        .substring(0, VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MAX);
      const sanitizedContent = formState.content
        ? formState.content.substring(
            0,
            VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX
          )
        : '';
      const sanitizedSolutionText = assets.solutionText
        ? assets.solutionText.substring(
            0,
            VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX
          )
        : '';
      const sanitizedCorrectAnswer = answerConfig.sanitizeCorrectAnswer(
        answerConfig.correctAnswer
      );

      const finalTagIds = await tagPicker.createPendingTagsForSubmit();
      const payload: Record<string, unknown> = {
        title: sanitizedTitle,
        content: sanitizedContent,
        problem_type: formState.problemType,
        correct_answer:
          formState.problemType === 'extended' ? '' : sanitizedCorrectAnswer,
        answer_config: answerConfig.answerConfig,
        auto_mark: formState.autoMarkValue,
        status: formState.status,
        assets: problemAssetsPayload,
        solution_text: sanitizedSolutionText,
        solution_assets: solutionAssetsPayload,
        tag_ids: finalTagIds,
      };

      if (!isEditMode) {
        payload.subject_id = subjectId;
        payload.id = assets.problemUuid;
      }

      const validatedPayload = validatePayload(
        payload,
        isEditMode ? UpdateProblemDto : CreateProblemDto,
        `${isEditMode ? 'update' : 'create'} problem`
      );
      const savedProblem = problem
        ? await updateProblem.mutateAsync({
            problemId: problem.id,
            payload: validatedPayload,
          })
        : await createProblem.mutateAsync(validatedPayload);

      toast.success(isEditMode ? t('problemUpdated') : t('problemCreated'));

      if (isEditMode) {
        onProblemUpdated?.(savedProblem);
        onCancel?.();
      } else {
        onProblemCreated?.(savedProblem);
        formState.resetAfterCreate();
        assets.resetAfterCreate();
        answerConfig.resetAfterCreate();
        tagPicker.resetAfterCreate();
      }
    } catch (error) {
      console.error('[problem-form] handleSubmit failed:', error);
      toast.error(
        error instanceof Error
          ? error.message
          : `Failed to ${isEditMode ? 'update' : 'create'} problem`
      );
    } finally {
      formState.setIsSubmitting(false);
    }
  }

  if (!formState.isExpanded && !isEditMode && !alwaysExpanded) {
    return (
      <ProblemFormCollapsed
        subjectId={subjectId}
        showImageScan={formState.showImageScan}
        extractionQuota={aiExtraction.extractionQuota}
        setExtractionQuota={aiExtraction.setExtractionQuota}
        onExtracted={aiExtraction.handleExtractionComplete}
        onManual={() => formState.setIsExpanded(true)}
        onScan={() => formState.setShowImageScan(true)}
        onCancelScan={() => formState.setShowImageScan(false)}
      />
    );
  }

  if (alwaysExpanded && !isEditMode && formState.showImageScan) {
    return (
      <ProblemFormScan
        subjectId={subjectId}
        extractionQuota={aiExtraction.extractionQuota}
        setExtractionQuota={aiExtraction.setExtractionQuota}
        onExtracted={aiExtraction.handleExtractionComplete}
        onCancel={() => onCancel?.()}
      />
    );
  }

  return (
    <ProblemFormFull
      problem={problem}
      isEditMode={isEditMode}
      alwaysExpanded={alwaysExpanded}
      onCancel={onCancel}
      onSubmit={handleSubmit}
      formState={formState}
      answerConfig={answerConfig}
      tagPicker={tagPicker}
      assets={assets}
      contentEditorRef={contentEditorRef}
      solutionEditorRef={solutionEditorRef}
    />
  );
}

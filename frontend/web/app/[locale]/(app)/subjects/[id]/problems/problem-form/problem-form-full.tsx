'use client';

import type { FormEvent, RefObject } from 'react';
import { Plus } from 'lucide-react';
import { useTranslations } from 'next-intl';
import FileManager from '@/components/ui/file-manager';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { StatusBadge } from '@/components/ui/status-badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  PROBLEM_TYPE_VALUES,
  type ProblemType,
} from '@/lib/validation/schemas';
import { getProblemTypeDisplayName } from '@/lib/utils/common';
import { RichTextEditor, type RichTextEditorHandle } from '@/components/editor';
import { MCQChoiceEditor } from '@/components/ui/mcq-choice-editor';
import { ShortAnswerConfig } from '@/components/ui/short-answer-config';
import { VALIDATION_CONSTANTS } from '@/lib/constants';
import { Spinner } from '@/components/ui/spinner';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion';
import type { Problem } from '@/lib/types';
import type { ProblemFormState } from './use-problem-form-state';
import type { AnswerConfigState } from './use-answer-config';
import type { TagPickerState } from './use-tag-picker';
import type { ProblemAssetsState } from './use-problem-assets';

interface ProblemFormFullProps {
  problem: Problem | null;
  isEditMode: boolean;
  alwaysExpanded: boolean;
  onCancel?: (() => void) | null;
  onSubmit: (event: FormEvent) => void;
  formState: ProblemFormState;
  answerConfig: AnswerConfigState;
  tagPicker: TagPickerState;
  assets: ProblemAssetsState;
  contentEditorRef: RefObject<RichTextEditorHandle | null>;
  solutionEditorRef: RefObject<RichTextEditorHandle | null>;
}

export function ProblemFormFull({
  problem,
  isEditMode,
  alwaysExpanded,
  onCancel,
  onSubmit,
  formState,
  answerConfig,
  tagPicker,
  assets,
  contentEditorRef,
  solutionEditorRef,
}: ProblemFormFullProps) {
  const t = useTranslations('Subjects');
  const tCommon = useTranslations('Common');
  const tProblems = useTranslations('Problems');
  const problemId = isEditMode
    ? problem?.id || 'disabled'
    : assets.problemUuid || 'disabled';

  return (
    <form onSubmit={onSubmit} className="form-container">
      {(isEditMode || (alwaysExpanded && !isEditMode)) && (
        <div className="flex items-center justify-between">
          <h3 className="heading-xs">
            {isEditMode ? t('editProblem') : t('addNewProblem')}
          </h3>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => onCancel?.()}
            className="text-muted-foreground hover:text-foreground"
          >
            {tCommon('cancel')}
          </Button>
        </div>
      )}

      <div className="form-row">
        <label className="form-label">{tProblems('problemTitle')}</label>
        <div className="flex-1 relative">
          <Input
            type="text"
            className="form-input w-full"
            placeholder={tProblems('titlePlaceholder')}
            value={formState.title}
            onChange={e => formState.setTitle(e.target.value)}
            maxLength={VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MAX}
            required
            onFocus={() => formState.setTitleFocus(true)}
            onBlur={() => formState.setTitleFocus(false)}
          />
          {formState.titleFocus && (
            <span
              className="absolute bottom-1.5 right-3 text-xs text-muted-foreground pointer-events-none bg-background px-1"
              style={{ lineHeight: 1 }}
            >
              {formState.title.length}/
              {VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MAX}
            </span>
          )}
        </div>
      </div>

      <Accordion
        type="multiple"
        defaultValue={['content', 'settings', 'answer']}
      >
        <AccordionItem
          value="content"
          className="rounded-2xl border border-gray-200/40 dark:border-gray-700/30 bg-gradient-to-br from-gray-50 to-gray-100/50 dark:from-gray-800/40 dark:to-gray-700/20 px-4"
        >
          <AccordionTrigger className="hover:no-underline py-3">
            <span className="text-sm font-semibold text-gray-800 dark:text-gray-300">
              {tProblems('content')} <span className="text-red-500">*</span>
            </span>
          </AccordionTrigger>
          <AccordionContent className="space-y-4">
            <div className="form-row-start">
              <label className="form-label pt-2">{t('contentLabel')}</label>
              <div className="flex-1 relative">
                <RichTextEditor
                  key={`content-${formState.editorKey}`}
                  ref={contentEditorRef}
                  initialContent={formState.content}
                  onChange={formState.setContent}
                  placeholder={tProblems('contentPlaceholder')}
                  height="200px"
                  maxHeight="500px"
                  disabled={formState.isSubmitting}
                  maxLength={VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX}
                  showCharacterCount={true}
                />
              </div>
            </div>
            <div className="form-row-start">
              <label className="form-label pt-2">{t('problemAssets')}</label>
              <div className="flex-1">
                <FileManager
                  role="problem"
                  problemId={problemId}
                  isEditMode={isEditMode}
                  initialFiles={assets.problemAssets}
                  onFilesChange={assets.setProblemAssets}
                  onInsertImage={assets.handleInsertProblemImage}
                  disabled={!isEditMode && !assets.problemUuid}
                />
              </div>
            </div>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem
          value="settings"
          className="rounded-2xl border border-amber-200/40 dark:border-amber-800/30 bg-gradient-to-br from-amber-50 to-amber-100/50 dark:from-amber-950/40 dark:to-amber-900/20 px-4 mt-4"
        >
          <AccordionTrigger className="hover:no-underline py-3">
            <span className="text-sm font-semibold text-amber-800 dark:text-amber-300">
              {tProblems('problemSettings')}{' '}
              <span className="text-red-500">*</span>
            </span>
          </AccordionTrigger>
          <AccordionContent>
            <div className="form-section">
              <div className="form-row">
                <label className="form-label">{tProblems('type')}</label>
                <Select
                  value={formState.problemType}
                  onValueChange={value =>
                    formState.setProblemType(value as ProblemType)
                  }
                >
                  <SelectTrigger className="w-48 rounded-xl">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PROBLEM_TYPE_VALUES.map(type => (
                      <SelectItem key={type} value={type}>
                        {tProblems(getProblemTypeDisplayName(type))}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="form-row">
                <label className="form-label">{tProblems('status')}</label>
                <Select
                  value={formState.status}
                  onValueChange={value => formState.setStatus(value as any)}
                >
                  <SelectTrigger className="w-36 rounded-xl">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="needs_review">
                      <StatusBadge status="needs_review" t={tProblems} />
                    </SelectItem>
                    <SelectItem value="wrong">
                      <StatusBadge status="wrong" t={tProblems} />
                    </SelectItem>
                    <SelectItem value="mastered">
                      <StatusBadge status="mastered" t={tProblems} />
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="form-row">
                <div className="flex items-center gap-2">
                  <Switch
                    id="auto-mark-switch"
                    checked={formState.autoMarkValue}
                    disabled={formState.isAutoMarkDisabled}
                    onCheckedChange={formState.setAutoMark}
                  />
                  <Label
                    htmlFor="auto-mark-switch"
                    className={`text-sm cursor-pointer ${formState.isAutoMarkDisabled ? 'text-muted-foreground' : ''}`}
                  >
                    {tProblems('autoMark')}
                    {formState.isAutoMarkDisabled && (
                      <span className="text-body-sm text-muted-foreground ml-1">
                        {t('autoMarkNotAvailable')}
                      </span>
                    )}
                  </Label>
                </div>
              </div>
            </div>
          </AccordionContent>
        </AccordionItem>

        {formState.problemType === 'mcq' && (
          <AccordionItem
            value="answer"
            className="rounded-2xl border border-blue-200/40 dark:border-blue-800/30 bg-gradient-to-br from-blue-50 to-blue-100/50 dark:from-blue-950/40 dark:to-blue-900/20 px-4 mt-4"
          >
            <AccordionTrigger className="hover:no-underline py-3">
              <span className="text-sm font-semibold text-blue-800 dark:text-blue-300">
                {tProblems('answerConfig')}
              </span>
            </AccordionTrigger>
            <AccordionContent>
              <div className="form-section">
                <div className="form-row">
                  <span className="form-label">{tProblems('answerMode')}</span>
                  <div className="flex items-center gap-2">
                    <Switch
                      id="enhanced-mcq-switch"
                      checked={answerConfig.useEnhancedMcq}
                      onCheckedChange={answerConfig.setUseEnhancedMcq}
                    />
                    <Label
                      htmlFor="enhanced-mcq-switch"
                      className="text-sm cursor-pointer"
                    >
                      {t('useChoicePicker')}
                    </Label>
                  </div>
                </div>

                {answerConfig.useEnhancedMcq && (
                  <div className="form-row">
                    <span className="form-label" />
                    <div className="flex items-center gap-2">
                      <Switch
                        id="randomize-choices-switch"
                        checked={answerConfig.mcqRandomizeChoices}
                        onCheckedChange={answerConfig.setMcqRandomizeChoices}
                        disabled={formState.isSubmitting}
                      />
                      <Label
                        htmlFor="randomize-choices-switch"
                        className="text-sm cursor-pointer"
                      >
                        {t('randomizeChoices')}
                      </Label>
                    </div>
                  </div>
                )}

                {answerConfig.useEnhancedMcq ? (
                  <MCQChoiceEditor
                    choices={answerConfig.mcqChoices}
                    correctChoiceId={answerConfig.mcqCorrectChoiceId}
                    onChoicesChange={answerConfig.setMcqChoices}
                    onCorrectChoiceChange={answerConfig.setMcqCorrectChoiceId}
                    disabled={formState.isSubmitting}
                  />
                ) : (
                  <div className="form-row">
                    <label className="form-label">{t('correctChoice')}</label>
                    <Input
                      className="form-input w-32"
                      placeholder={t('correctChoicePlaceholder')}
                      value={answerConfig.mcqChoice}
                      maxLength={
                        VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX
                      }
                      onChange={e => answerConfig.setMcqChoice(e.target.value)}
                    />
                  </div>
                )}
              </div>
            </AccordionContent>
          </AccordionItem>
        )}

        {formState.problemType === 'short' && (
          <AccordionItem
            value="answer"
            className="rounded-2xl border border-rose-200/40 dark:border-rose-800/30 bg-gradient-to-br from-rose-50 to-rose-100/50 dark:from-rose-950/40 dark:to-rose-900/20 px-4 mt-4"
          >
            <AccordionTrigger className="hover:no-underline py-3">
              <span className="text-sm font-semibold text-rose-800 dark:text-rose-300">
                {tProblems('answerConfig')}
              </span>
            </AccordionTrigger>
            <AccordionContent>
              <div className="form-section">
                <div className="form-row">
                  <div className="flex items-center gap-2">
                    <Switch
                      id="enhanced-short-switch"
                      checked={answerConfig.useEnhancedShort}
                      onCheckedChange={answerConfig.setUseEnhancedShort}
                    />
                    <Label
                      htmlFor="enhanced-short-switch"
                      className="text-sm cursor-pointer"
                    >
                      {t('advancedMode')}
                    </Label>
                  </div>
                </div>

                {answerConfig.useEnhancedShort ? (
                  <ShortAnswerConfig
                    value={answerConfig.shortAnswerConfig}
                    onChange={answerConfig.setShortAnswerConfig}
                    disabled={formState.isSubmitting}
                  />
                ) : (
                  <div className="form-row">
                    <label className="form-label">{t('correctText')}</label>
                    <Input
                      className="form-input"
                      placeholder={t('correctTextPlaceholder')}
                      value={answerConfig.shortText}
                      maxLength={
                        VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX
                      }
                      onChange={e => answerConfig.setShortText(e.target.value)}
                    />
                  </div>
                )}
              </div>
            </AccordionContent>
          </AccordionItem>
        )}

        <AccordionItem
          value="solution"
          className="rounded-2xl border border-green-200/40 dark:border-green-800/30 bg-gradient-to-br from-green-50 to-green-100/50 dark:from-green-950/40 dark:to-green-900/20 px-4 mt-4"
        >
          <AccordionTrigger className="hover:no-underline py-3">
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-green-800 dark:text-green-300">
                {tProblems('solution')}
              </span>
            </div>
          </AccordionTrigger>
          <AccordionContent className="space-y-4">
            <div className="form-row-start">
              <label className="form-label pt-2">
                {tProblems('solutionText')}
              </label>
              <div className="flex-1 relative">
                <RichTextEditor
                  key={`solution-${formState.editorKey}`}
                  ref={solutionEditorRef}
                  initialContent={assets.solutionText}
                  onChange={assets.setSolutionText}
                  placeholder={tProblems('solutionPlaceholder')}
                  height="200px"
                  maxHeight="500px"
                  disabled={formState.isSubmitting}
                  maxLength={VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX}
                  showCharacterCount={true}
                />
              </div>
            </div>
            <div className="form-row-start">
              <label className="form-label pt-2">{t('solutionAssets')}</label>
              <div className="flex-1">
                <FileManager
                  role="solution"
                  problemId={problemId}
                  isEditMode={isEditMode}
                  initialFiles={assets.solutionAssets}
                  onFilesChange={assets.setSolutionAssets}
                  onInsertImage={assets.handleInsertSolutionImage}
                  disabled={!isEditMode && !assets.problemUuid}
                />
              </div>
            </div>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem
          value="tags"
          className="rounded-2xl border border-gray-200/40 dark:border-gray-700/30 bg-gradient-to-br from-gray-50 to-gray-100/50 dark:from-gray-800/40 dark:to-gray-700/20 px-4 mt-4"
        >
          <AccordionTrigger className="hover:no-underline py-3">
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold text-gray-800 dark:text-gray-300">
                {tCommon('tags')}
              </span>
              {(() => {
                const selectedPendingCount = tagPicker.pendingNewTags.filter(
                  name => !tagPicker.deselectedPendingTags.has(name)
                ).length;
                const total =
                  tagPicker.selectedTagIds.length + selectedPendingCount;
                return (
                  total > 0 && (
                    <span className="text-xs text-amber-600 dark:text-amber-400">
                      {t('selectedCount', { count: total })}
                      {selectedPendingCount > 0 &&
                        ` ${t('newCount', { count: selectedPendingCount })}`}
                    </span>
                  )
                );
              })()}
            </div>
          </AccordionTrigger>
          <AccordionContent>
            <div className="space-y-3">
              <div className="flex flex-wrap gap-2">
                {tagPicker.tags.length ? (
                  tagPicker.tags.map(tag => {
                    const selected = tagPicker.selectedTagIds.includes(tag.id);
                    return (
                      <button
                        key={tag.id}
                        type="button"
                        onClick={() => tagPicker.toggleTag(tag.id)}
                        className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-sm transition-colors ${
                          selected
                            ? 'bg-amber-100 dark:bg-amber-900/40 text-amber-800 dark:text-amber-300 border border-amber-300/60 dark:border-amber-700/50'
                            : 'bg-gray-100/80 dark:bg-gray-800/40 text-gray-600 dark:text-gray-400 border border-gray-200/50 dark:border-gray-700/40 hover:bg-gray-200/80 dark:hover:bg-gray-700/40'
                        }`}
                      >
                        {tag.name}
                      </button>
                    );
                  })
                ) : (
                  <p className="text-body-sm text-muted-foreground">
                    {t('noTagsYet')}
                  </p>
                )}
              </div>
              {tagPicker.pendingNewTags.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {tagPicker.pendingNewTags.map(name => {
                    const selected = !tagPicker.deselectedPendingTags.has(name);
                    return (
                      <button
                        key={`pending-${name}`}
                        type="button"
                        aria-pressed={selected}
                        onClick={() =>
                          tagPicker.setDeselectedPendingTags(prev => {
                            const next = new Set(prev);
                            if (next.has(name)) next.delete(name);
                            else next.add(name);
                            return next;
                          })
                        }
                        className={`inline-flex items-center gap-1 rounded-full px-3 py-1 text-sm border border-dashed transition-colors ${
                          selected
                            ? 'bg-blue-50/80 text-blue-700 border-blue-300/60 dark:bg-blue-900/20 dark:text-blue-300 dark:border-blue-700/40 hover:bg-blue-100/80 dark:hover:bg-blue-900/40'
                            : 'bg-gray-100/80 text-gray-500 border-gray-300/50 dark:bg-gray-800/40 dark:text-gray-500 dark:border-gray-700/40 hover:bg-gray-200/80 dark:hover:bg-gray-700/40'
                        }`}
                      >
                        {name}
                        <span className="text-[10px] font-medium opacity-70 ml-0.5">
                          {t('newTag')}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}
              <div className="flex items-center gap-2 border-t border-gray-200/40 dark:border-gray-700/30 pt-3">
                <Input
                  placeholder={t('newTagPlaceholder')}
                  value={tagPicker.newTagName}
                  onChange={e => tagPicker.setNewTagName(e.target.value)}
                  onKeyDown={e => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      tagPicker.handleCreateTag();
                    }
                  }}
                  disabled={tagPicker.creatingTag}
                  className="h-8 flex-1 text-sm"
                />
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={tagPicker.handleCreateTag}
                  disabled={
                    tagPicker.creatingTag || !tagPicker.newTagName.trim()
                  }
                >
                  {tagPicker.creatingTag ? (
                    <Spinner />
                  ) : (
                    <Plus className="h-3.5 w-3.5" />
                  )}
                  {t('addTag')}
                </Button>
              </div>
            </div>
          </AccordionContent>
        </AccordionItem>
      </Accordion>

      <div className="form-actions">
        <Button type="submit" disabled={formState.isSubmitting}>
          {formState.isSubmitting && <Spinner />}
          {formState.isSubmitting
            ? isEditMode
              ? t('updating')
              : t('adding')
            : isEditMode
              ? t('editProblem')
              : t('addProblem')}
        </Button>
        {!isEditMode && (
          <Button
            type="button"
            variant="outline"
            onClick={async () => {
              if (assets.problemUuid) {
                await assets.cleanupUnsavedProblem(assets.problemUuid);
              }
              assets.setProblemUuid(null);
              if (alwaysExpanded && onCancel) {
                onCancel();
              } else {
                formState.setIsExpanded(false);
              }
            }}
            disabled={formState.isSubmitting}
          >
            {tCommon('cancel')}
          </Button>
        )}
      </div>
    </form>
  );
}

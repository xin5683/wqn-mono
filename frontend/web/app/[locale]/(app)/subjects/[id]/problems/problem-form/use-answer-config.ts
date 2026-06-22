'use client';

import { useEffect, useMemo, useState } from 'react';
import { ANSWER_CONFIG_CONSTANTS, VALIDATION_CONSTANTS } from '@/lib/constants';
import type { ProblemType } from '@/lib/validation/schemas';
import type {
  AnswerConfig,
  ExtractedProblemData,
  MCQAnswerConfig,
  MCQChoice,
  Problem,
  ShortAnswerNumericConfig,
  ShortAnswerTextConfig,
} from '@/lib/types';
import type { ShortAnswerConfigValue } from '@/components/ui/short-answer-config';

interface UseAnswerConfigOptions {
  problem: Problem | null;
  problemType: ProblemType;
  isEditMode: boolean;
}

type AnswerValidationMessageKey =
  | 'correctChoiceRequired'
  | 'addAtLeastOneAnswer'
  | 'fillCorrectValueAndTolerance';

function defaultMcqChoices(): MCQChoice[] {
  return ANSWER_CONFIG_CONSTANTS.MCQ.DEFAULT_CHOICES.map(id => ({
    id,
    text: '',
  }));
}

export function useAnswerConfig({
  problem,
  problemType,
  isEditMode,
}: UseAnswerConfigOptions) {
  const [mcqChoice, setMcqChoice] = useState(
    typeof problem?.correct_answer === 'string' ? problem.correct_answer : ''
  );
  const [shortText, setShortText] = useState(
    typeof problem?.correct_answer === 'string' ? problem.correct_answer : ''
  );
  const [mcqChoices, setMcqChoices] = useState<MCQChoice[]>(() => {
    const config = problem?.answer_config;
    if (config && config.type === 'mcq') {
      return (config as MCQAnswerConfig).choices;
    }
    return defaultMcqChoices();
  });
  const [mcqCorrectChoiceId, setMcqCorrectChoiceId] = useState(() => {
    const config = problem?.answer_config;
    if (config && config.type === 'mcq') {
      return (config as MCQAnswerConfig).correct_choice_id;
    }
    return '';
  });
  const [mcqRandomizeChoices, setMcqRandomizeChoices] = useState(() => {
    const config = problem?.answer_config;
    if (config && config.type === 'mcq') {
      return (config as MCQAnswerConfig).randomize_choices ?? true;
    }
    return true;
  });
  const [shortAnswerConfig, setShortAnswerConfig] =
    useState<ShortAnswerConfigValue>(() => {
      const config = problem?.answer_config;
      if (config && config.type === 'short') {
        if ((config as ShortAnswerTextConfig).mode === 'text') {
          return {
            mode: 'text' as const,
            acceptable_answers: (config as ShortAnswerTextConfig)
              .acceptable_answers,
          };
        }
        if ((config as ShortAnswerNumericConfig).mode === 'numeric') {
          const nc = (config as ShortAnswerNumericConfig).numeric_config;
          return {
            mode: 'numeric' as const,
            numeric_config: {
              correct_value: nc.correct_value,
              tolerance: nc.tolerance,
              unit: nc.unit,
            },
          };
        }
      }
      return { mode: 'text' as const, acceptable_answers: [] };
    });
  const [useEnhancedMcq, setUseEnhancedMcq] = useState(() => {
    if (problem?.answer_config?.type === 'mcq') return true;
    if (!isEditMode) return true;
    return false;
  });
  const [useEnhancedShort, setUseEnhancedShort] = useState(
    !!(problem?.answer_config?.type === 'short')
  );

  useEffect(() => {
    if (!isEditMode && problemType === 'mcq') {
      setUseEnhancedMcq(true);
    }
  }, [problemType, isEditMode]);

  const correctAnswer = useMemo(() => {
    switch (problemType) {
      case 'mcq':
        if (useEnhancedMcq && mcqCorrectChoiceId) return mcqCorrectChoiceId;
        return mcqChoice;
      case 'short':
        if (useEnhancedShort && shortAnswerConfig.mode === 'text') {
          return shortAnswerConfig.acceptable_answers[0] || '';
        }
        if (
          useEnhancedShort &&
          shortAnswerConfig.mode === 'numeric' &&
          shortAnswerConfig.numeric_config.correct_value !== ''
        ) {
          return String(shortAnswerConfig.numeric_config.correct_value);
        }
        return shortText;
      case 'extended':
        return undefined;
    }
  }, [
    problemType,
    mcqChoice,
    shortText,
    useEnhancedMcq,
    useEnhancedShort,
    mcqCorrectChoiceId,
    shortAnswerConfig,
  ]);

  const answerConfig = useMemo((): AnswerConfig | null => {
    if (problemType === 'mcq' && useEnhancedMcq) {
      if (!mcqCorrectChoiceId) return null;
      return {
        type: 'mcq',
        choices: mcqChoices,
        correct_choice_id: mcqCorrectChoiceId,
        randomize_choices: mcqRandomizeChoices,
      };
    }
    if (problemType === 'short' && useEnhancedShort) {
      if (shortAnswerConfig.mode === 'text') {
        if (shortAnswerConfig.acceptable_answers.length === 0) return null;
        return {
          type: 'short',
          mode: 'text',
          acceptable_answers: shortAnswerConfig.acceptable_answers,
        };
      }
      if (shortAnswerConfig.mode === 'numeric') {
        const nc = shortAnswerConfig.numeric_config;
        if (nc.correct_value === '' || nc.tolerance === '') return null;
        return {
          type: 'short',
          mode: 'numeric',
          numeric_config: {
            correct_value: Number(nc.correct_value),
            tolerance: Number(nc.tolerance),
            unit: nc.unit || undefined,
          },
        };
      }
    }
    return null;
  }, [
    problemType,
    useEnhancedMcq,
    useEnhancedShort,
    mcqChoices,
    mcqCorrectChoiceId,
    mcqRandomizeChoices,
    shortAnswerConfig,
  ]);

  function applyExtractedAnswerHint(
    data: ExtractedProblemData,
    setSolutionHtml: (html: string) => void,
    toEditorHtml: (text: string) => string
  ) {
    if (
      data.problem_type === 'mcq' &&
      data.mcq_choices &&
      data.mcq_choices.length > 0
    ) {
      setUseEnhancedMcq(true);
      setMcqChoices(data.mcq_choices);
      setMcqCorrectChoiceId('');
    }

    const hint = data.answer_hint;
    if (!hint) return;

    if (data.problem_type === 'mcq' && hint.mcq_correct_choice_id) {
      setMcqCorrectChoiceId(hint.mcq_correct_choice_id);
    }

    if (data.problem_type === 'short' && hint.short_answer_value) {
      setUseEnhancedShort(true);
      if (hint.short_answer_is_numeric) {
        const numVal = Number(hint.short_answer_value);
        if (!isNaN(numVal)) {
          setShortAnswerConfig({
            mode: 'numeric',
            numeric_config: {
              correct_value: numVal,
              tolerance: 0,
              unit: '',
            },
          });
        } else {
          setShortAnswerConfig({
            mode: 'text',
            acceptable_answers: [hint.short_answer_value],
          });
        }
      } else {
        setShortAnswerConfig({
          mode: 'text',
          acceptable_answers: [hint.short_answer_value],
        });
      }
    }

    if (data.problem_type === 'extended' && hint.extended_working) {
      setSolutionHtml(toEditorHtml(hint.extended_working));
    }
  }

  function validateForSubmit(
    t: (key: AnswerValidationMessageKey) => string
  ): boolean {
    if (problemType === 'mcq' && useEnhancedMcq && !mcqCorrectChoiceId) {
      throw new Error(t('correctChoiceRequired'));
    }
    if (problemType === 'short' && useEnhancedShort) {
      if (
        shortAnswerConfig.mode === 'text' &&
        shortAnswerConfig.acceptable_answers.length === 0
      ) {
        throw new Error(t('addAtLeastOneAnswer'));
      }
      if (shortAnswerConfig.mode === 'numeric') {
        const nc = shortAnswerConfig.numeric_config;
        if (nc.correct_value === '' || nc.tolerance === '') {
          throw new Error(t('fillCorrectValueAndTolerance'));
        }
      }
    }
    return true;
  }

  function resetAfterCreate() {
    setShortText('');
    setMcqChoice('');
    setMcqChoices(defaultMcqChoices());
    setMcqCorrectChoiceId('');
    setMcqRandomizeChoices(true);
    setShortAnswerConfig({
      mode: 'text',
      acceptable_answers: [],
    });
    setUseEnhancedMcq(false);
    setUseEnhancedShort(false);
  }

  function sanitizeCorrectAnswer(value: string | undefined): string {
    return value
      ? value.substring(0, VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX)
      : '';
  }

  return {
    mcqChoice,
    setMcqChoice,
    shortText,
    setShortText,
    mcqChoices,
    setMcqChoices,
    mcqCorrectChoiceId,
    setMcqCorrectChoiceId,
    mcqRandomizeChoices,
    setMcqRandomizeChoices,
    shortAnswerConfig,
    setShortAnswerConfig,
    useEnhancedMcq,
    setUseEnhancedMcq,
    useEnhancedShort,
    setUseEnhancedShort,
    correctAnswer,
    answerConfig,
    applyExtractedAnswerHint,
    validateForSubmit,
    resetAfterCreate,
    sanitizeCorrectAnswer,
  };
}

export type AnswerConfigState = ReturnType<typeof useAnswerConfig>;

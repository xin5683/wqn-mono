import { describe, expect, it } from 'vitest';
import { ProblemListResponseSchema } from '../api/response-schemas';

// 后端 ProblemDto（src/dto/problem.rs）序列化后的典型形态。这个 fixture
// 是前后端契约的快照：若后端改动 DTO 导致字段缺失/类型漂移，这里的
// safeParse 会失败，把契约漂移挡在测试阶段，而不是让它变成运行时
// “刷新后列表为空”（曾因 loadData 静默吞错而难以发现）。
const baseProblem = {
  id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  user_id: 'bbbbbbbb-bbbb-4bbb-9bbb-bbbbbbbbbbbb',
  subject_id: 'cccccccc-cccc-4ccc-accc-cccccccccccc',
  title: 'Sample problem',
  content: '<p>content</p>',
  problem_type: 'extended',
  correct_answer: '',
  auto_mark: false,
  status: 'needs_review',
  assets: [],
  solution_text: null,
  solution_assets: [],
  last_reviewed_date: null,
  created_at: '2026-06-21T00:00:00Z',
  updated_at: '2026-06-21T00:00:00Z',
  tags: [],
};

describe('ProblemListResponseSchema contract', () => {
  it('accepts an extended problem with null answer_config', () => {
    const result = ProblemListResponseSchema.safeParse([
      { ...baseProblem, answer_config: null },
    ]);
    expect(result.success).toBe(true);
  });

  it('accepts a problem with a missing answer_config key', () => {
    const result = ProblemListResponseSchema.safeParse([baseProblem]);
    expect(result.success).toBe(true);
  });

  it('accepts an mcq answer_config', () => {
    const result = ProblemListResponseSchema.safeParse([
      {
        ...baseProblem,
        problem_type: 'mcq',
        answer_config: {
          type: 'mcq',
          choices: [
            { id: 'a', text: 'Option A' },
            { id: 'b', text: 'Option B' },
          ],
          correct_choice_id: 'a',
          randomize_choices: true,
        },
      },
    ]);
    expect(result.success).toBe(true);
  });

  it('accepts a short-text answer_config', () => {
    const result = ProblemListResponseSchema.safeParse([
      {
        ...baseProblem,
        problem_type: 'short',
        answer_config: {
          type: 'short',
          mode: 'text',
          acceptable_answers: ['42'],
        },
      },
    ]);
    expect(result.success).toBe(true);
  });

  it('accepts a short-numeric answer_config', () => {
    const result = ProblemListResponseSchema.safeParse([
      {
        ...baseProblem,
        problem_type: 'short',
        answer_config: {
          type: 'short',
          mode: 'numeric',
          numeric_config: { correct_value: 9.8, tolerance: 0.1, unit: 'm/s²' },
        },
      },
    ]);
    expect(result.success).toBe(true);
  });

  it('rejects a legacy answer_config shape the backend must not emit', () => {
    // 旧形态（如用 options 而非 choices）不应再被后端写入；若出现说明契约漂移。
    const result = ProblemListResponseSchema.safeParse([
      {
        ...baseProblem,
        answer_config: {
          type: 'mcq',
          options: [{ id: 'a', text: 'A' }],
        },
      },
    ]);
    expect(result.success).toBe(false);
  });

  it('rejects an mcq answer_config whose correct_choice_id is not in choices', () => {
    const result = ProblemListResponseSchema.safeParse([
      {
        ...baseProblem,
        problem_type: 'mcq',
        answer_config: {
          type: 'mcq',
          choices: [{ id: 'a', text: 'A' }],
          correct_choice_id: 'zzz',
        },
      },
    ]);
    expect(result.success).toBe(false);
  });
});

import { z } from 'zod';
import {
  AnswerConfigSchema,
  FilterConfigSchema,
  ProblemSetSharingLevel,
  ProblemStatus,
  ProblemType,
  SessionConfigSchema,
} from '@/lib/validation/schemas';

const DateTimeString = z.string();
const NullableString = z.string().nullable();
const NullableDateTimeString = DateTimeString.nullable();
const OptionalString = z.preprocess(
  value => (value === null ? undefined : value),
  z.string().optional()
);

export const UserProfileResponseSchema = z.object({
  id: z.uuid(),
  username: NullableString.optional(),
  first_name: NullableString.optional(),
  last_name: NullableString.optional(),
  date_of_birth: NullableString.optional(),
  gender: NullableString.optional(),
  region: NullableString.optional(),
  timezone: z.string().optional(),
  avatar_url: NullableString.optional(),
  bio: NullableString.optional(),
  user_role: z.string().optional(),
  is_active: z.boolean().optional(),
  onboarding_completed_at: NullableDateTimeString.optional(),
  last_login_at: NullableDateTimeString.optional(),
  created_at: DateTimeString.optional(),
  updated_at: DateTimeString.optional(),
});

export const CurrentUserResponseSchema = z.object({
  id: z.uuid(),
  email: z.string(),
  disabled_at: NullableDateTimeString.optional(),
  last_login_at: NullableDateTimeString.optional(),
  created_at: DateTimeString.optional(),
  updated_at: DateTimeString.optional(),
  role: z.string(),
  is_admin: z.boolean(),
  profile: UserProfileResponseSchema.nullable(),
});

export type CurrentUserResponse = z.infer<typeof CurrentUserResponseSchema>;

export const SubjectResponseSchema = z.object({
  id: z.uuid(),
  name: z.string(),
  color: OptionalString,
  icon: OptionalString,
  created_at: DateTimeString.optional(),
  updated_at: DateTimeString.optional(),
  problem_count: z.number().optional(),
  tag_count: z.number().optional(),
  due_count: z.number().optional(),
});
export type SubjectResponse = z.infer<typeof SubjectResponseSchema>;

export const SubjectListResponseSchema = z.array(SubjectResponseSchema);

export const TagResponseSchema = z.object({
  id: z.uuid(),
  name: z.string(),
  subject_id: z.uuid(),
  user_id: z.uuid().optional(),
  created_at: DateTimeString,
  updated_at: DateTimeString.optional(),
});
export type TagResponse = z.infer<typeof TagResponseSchema>;

export const AssetResponseSchema = z.object({
  path: z.string(),
  kind: z.enum(['image', 'pdf']).optional(),
});
export type AssetResponse = z.infer<typeof AssetResponseSchema>;

export const ProblemResponseSchema = z.object({
  id: z.uuid(),
  user_id: z.uuid().optional(),
  subject_id: z.uuid(),
  title: z.string(),
  content: NullableString,
  problem_type: ProblemType,
  correct_answer: NullableString,
  answer_config: AnswerConfigSchema.nullable().optional(),
  auto_mark: z.boolean(),
  status: ProblemStatus,
  assets: z.array(AssetResponseSchema).optional(),
  solution_text: NullableString.optional(),
  solution_assets: z.array(AssetResponseSchema).optional(),
  last_reviewed_date: NullableDateTimeString.optional(),
  created_at: DateTimeString,
  updated_at: DateTimeString,
  tags: z.array(TagResponseSchema).optional(),
});
export type ProblemResponse = z.infer<typeof ProblemResponseSchema>;

export const ProblemListResponseSchema = z.array(ProblemResponseSchema);

export const AttemptResponseSchema = z.object({
  id: z.uuid(),
  problem_id: z.uuid(),
  submitted_answer: z.union([
    z.string(),
    z.number(),
    z.boolean(),
    z.record(z.string(), z.unknown()),
  ]),
  is_correct: z.boolean().nullable(),
  cause: NullableString,
  is_self_assessed: z.boolean(),
  confidence: z.number().nullable(),
  reflection_notes: NullableString,
  selected_status: ProblemStatus.nullable(),
  created_at: DateTimeString,
  updated_at: DateTimeString,
});
export type AttemptResponse = z.infer<typeof AttemptResponseSchema>;

export const ProblemSetShareResponseSchema = z.object({
  id: z.uuid(),
  shared_with_email: z.string(),
  created_at: DateTimeString.optional(),
});
export type ProblemSetShareResponse = z.infer<
  typeof ProblemSetShareResponseSchema
>;

export const ProblemSetOwnerProfileResponseSchema = z.object({
  id: z.uuid().optional(),
  username: NullableString,
  first_name: NullableString,
  last_name: NullableString,
  avatar_url: NullableString,
  bio: NullableString,
  gender: NullableString,
});
export type ProblemSetOwnerProfileResponse = z.infer<
  typeof ProblemSetOwnerProfileResponseSchema
>;

export const ProblemSetResponseSchema = z.object({
  id: z.uuid(),
  user_id: z.uuid().optional(),
  subject_id: z.uuid(),
  name: z.string(),
  description: NullableString,
  sharing_level: ProblemSetSharingLevel,
  is_smart: z.boolean(),
  filter_config: FilterConfigSchema.nullable().optional(),
  session_config: SessionConfigSchema.nullable().optional(),
  allow_copying: z.boolean(),
  is_listed: z.boolean(),
  discovery_subject: NullableString,
  created_at: DateTimeString,
  updated_at: DateTimeString,
  subject_name: z.string().optional(),
  problem_count: z.number().optional(),
  shared_with_emails: z.array(z.string()).optional(),
  problem_set_shares: z.array(ProblemSetShareResponseSchema).optional(),
  owner_profile: ProblemSetOwnerProfileResponseSchema.nullable().optional(),
  problems: z.array(ProblemResponseSchema).optional(),
  favorited_at: DateTimeString.optional(),
  favourited_at: DateTimeString.optional(),
});
export type ProblemSetResponse = z.infer<typeof ProblemSetResponseSchema>;

export const ProblemSetListResponseSchema = z.array(ProblemSetResponseSchema);

export const ProblemSetProblemResponseSchema = z.object({
  problem_id: z.uuid(),
  added_at: NullableDateTimeString.optional(),
  problems: ProblemResponseSchema,
});
export type ProblemSetProblemResponse = z.infer<
  typeof ProblemSetProblemResponseSchema
>;

export const ProblemSetDetailResponseSchema = ProblemSetResponseSchema.omit({
  problems: true,
}).extend({
  problems: z.array(ProblemSetProblemResponseSchema).optional(),
});
export type ProblemSetDetailResponse = z.infer<
  typeof ProblemSetDetailResponseSchema
>;

export const ProblemSetStatsResponseSchema = z.object({
  view_count: z.number(),
  unique_view_count: z.number().optional(),
  like_count: z.number(),
  favourite_count: z.number().optional(),
  favorite_count: z.number().optional(),
  copy_count: z.number(),
  problem_count: z.number().optional(),
  ranking_score: z.number().optional(),
  liked: z.boolean().optional(),
  favourited: z.boolean().optional(),
  favorited: z.boolean().optional(),
});
export type ProblemSetStatsResponse = z.infer<
  typeof ProblemSetStatsResponseSchema
>;

export const ProblemSetCardResponseSchema = z.object({
  id: z.uuid(),
  name: z.string(),
  description: NullableString,
  subject_name: z.string(),
  subject_color: NullableString,
  subject_icon: NullableString,
  problem_count: z.number(),
  is_smart: z.boolean(),
  owner: z.object({
    username: NullableString,
    display_name: z.string(),
    avatar_url: NullableString,
  }),
  stats: ProblemSetStatsResponseSchema.extend({
    unique_view_count: z.number(),
    problem_count: z.number(),
    ranking_score: z.number(),
  }),
  created_at: DateTimeString,
});
export type ProblemSetCardResponse = z.infer<
  typeof ProblemSetCardResponseSchema
>;

export const DiscoverProblemSetItemResponseSchema =
  ProblemSetResponseSchema.extend({
    user_id: z.uuid(),
    subject_name: z.string(),
    subject_color: NullableString,
    subject_icon: NullableString,
    problem_count: z.number(),
    owner_username: NullableString,
    owner_avatar_url: NullableString,
    view_count: z.number(),
    unique_view_count: z.number().optional(),
    like_count: z.number(),
    copy_count: z.number(),
    ranking_score: z.number(),
    owner: ProblemSetCardResponseSchema.shape.owner,
    stats: ProblemSetCardResponseSchema.shape.stats,
  });
export type DiscoverProblemSetItemResponse = z.infer<
  typeof DiscoverProblemSetItemResponseSchema
>;

export const DiscoverResponseSchema = z.object({
  items: z.array(DiscoverProblemSetItemResponseSchema).optional(),
  data: z.array(DiscoverProblemSetItemResponseSchema).optional(),
  next_cursor: NullableString.optional(),
  cursor: NullableString.optional(),
  sort: z.string().optional(),
});
export type DiscoverResponse = z.infer<typeof DiscoverResponseSchema>;

export const CreatorProfileResponseSchema = z.object({
  username: NullableString,
  display_name: z.string(),
  avatar_url: NullableString,
  bio: NullableString,
});
export type CreatorProfileResponse = z.infer<
  typeof CreatorProfileResponseSchema
>;

export const CreatorAggregateStatsResponseSchema = z.object({
  total_views: z.number(),
  total_likes: z.number(),
  total_copies: z.number(),
});
export type CreatorAggregateStatsResponse = z.infer<
  typeof CreatorAggregateStatsResponseSchema
>;

export const CreatorResponseSchema = z.object({
  profile: CreatorProfileResponseSchema,
  sets: z.array(ProblemSetCardResponseSchema),
  aggregateStats: CreatorAggregateStatsResponseSchema,
});
export type CreatorResponse = z.infer<typeof CreatorResponseSchema>;

export const ReviewSessionStateResponseSchema = z.object({
  id: z.uuid(),
  user_id: z.uuid(),
  problem_set_id: z.uuid().nullable(),
  session_type: z.enum(['normal', 'spaced_repetition', 'insights']).optional(),
  subject_id: z.uuid().nullable().optional(),
  started_at: DateTimeString,
  last_activity_at: DateTimeString,
  is_active: z.boolean(),
  session_state: z.object({
    problem_ids: z.array(z.uuid()),
    current_index: z.number(),
    completed_problem_ids: z.array(z.uuid()),
    skipped_problem_ids: z.array(z.uuid()),
    initial_statuses: z.record(z.string(), ProblemStatus).optional(),
    elapsed_ms: z.number(),
    is_read_only: z.boolean().optional(),
  }),
});
export type ReviewSessionStateResponse = z.infer<
  typeof ReviewSessionStateResponseSchema
>;

export const ReviewSessionResultResponseSchema = z.object({
  id: z.uuid(),
  session_state_id: z.uuid(),
  problem_id: z.uuid(),
  completed_at: DateTimeString,
  was_correct: z.boolean().nullable(),
  was_skipped: z.boolean(),
});
export type ReviewSessionResultResponse = z.infer<
  typeof ReviewSessionResultResponseSchema
>;

export const ReviewSessionResponseSchema = z.object({
  session: ReviewSessionStateResponseSchema.nullable().optional(),
  problems: z.array(ProblemResponseSchema).optional(),
  results: z.array(ReviewSessionResultResponseSchema).optional(),
});
export type ReviewSessionResponse = z.infer<typeof ReviewSessionResponseSchema>;

const InsightDigestBaseResponseSchema = z.object({
  id: z.uuid(),
  user_id: z.uuid(),
  status: z.enum(['generating', 'completed', 'failed']),
  error_message: NullableString.optional(),
  generated_at: NullableDateTimeString.optional(),
  created_at: DateTimeString,
  updated_at: DateTimeString,
});

export const WeakSpotResponseSchema = z.object({
  topic_label: z.string(),
  subject_id: z.uuid(),
  subject_name: z.string(),
  subject_color: z.string().optional(),
  problem_count: z.number(),
  trend_phrase: z.string(),
  dominant_error_type: z.string(),
  problem_ids: z.array(z.uuid()),
});
export type WeakSpotResponse = z.infer<typeof WeakSpotResponseSchema>;

export const TopicClusterResponseSchema = z.object({
  label: z.string(),
  problem_count: z.number(),
  wrong_count: z.number(),
  needs_review_count: z.number(),
  mastered_count: z.number(),
  narrative: z.string(),
  problem_ids: z.array(z.uuid()),
});
export type TopicClusterResponse = z.infer<typeof TopicClusterResponseSchema>;

export const InsightDigestResponseSchema =
  InsightDigestBaseResponseSchema.extend({
    status: z.literal('completed'),
    generated_at: DateTimeString,
    headline: z.string(),
    error_pattern_summary: z.string(),
    subject_error_patterns: z.record(z.string(), z.string()).optional(),
    subject_health: z.record(z.string(), z.string()),
    weak_spots: z.array(WeakSpotResponseSchema),
    topic_clusters: z.record(z.string(), z.array(TopicClusterResponseSchema)),
    progress_narratives: z.record(z.string(), z.string()),
    raw_aggregation_data: z.record(z.string(), z.unknown()).optional(),
    digest_tier: z.enum(['full', 'mastery', 'narrow']).optional(),
  });
export type InsightDigestResponse = z.infer<typeof InsightDigestResponseSchema>;

export const InsightStatusResponseSchema = z.object({
  latest: InsightDigestBaseResponseSchema.nullable(),
  is_generating: z.boolean(),
  status: z.string(),
  digest: InsightDigestResponseSchema.nullable(),
});

export const RegistrationStatusResponseSchema = z.object({
  enabled: z.boolean(),
});
export type RegistrationStatusResponse = z.infer<
  typeof RegistrationStatusResponseSchema
>;

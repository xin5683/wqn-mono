import { z } from 'zod';
import {
  PROBLEM_CONSTANTS,
  PROBLEM_SET_CONSTANTS,
  VALIDATION_CONSTANTS,
  ANSWER_CONFIG_CONSTANTS,
  USER_ROLES,
  GENDER_OPTIONS,
  SUBJECT_CONSTANTS,
  ATTEMPT_CONSTANTS,
  SPACED_REPETITION_CONSTANTS,
  ERROR_CATEGORY_VALUES,
  DISCOVERY_SUBJECTS,
} from '../constants';
import { sanitizeHtmlContent } from '../security/html-sanitizer';
import { isValidTimezone } from '../utils/timezone';

// Database enum values - these should match the PostgreSQL enum type
export const PROBLEM_TYPE_VALUES = [
  PROBLEM_CONSTANTS.TYPES.MCQ,
  PROBLEM_CONSTANTS.TYPES.SHORT,
  PROBLEM_CONSTANTS.TYPES.EXTENDED,
] as const;
export const PROBLEM_STATUS_VALUES = [
  PROBLEM_CONSTANTS.STATUS.WRONG,
  PROBLEM_CONSTANTS.STATUS.NEEDS_REVIEW,
  PROBLEM_CONSTANTS.STATUS.MASTERED,
] as const;

export const ProblemType = z.enum(PROBLEM_TYPE_VALUES);
export type ProblemType = z.infer<typeof ProblemType>;

export const ProblemStatus = z.enum(PROBLEM_STATUS_VALUES);
export type ProblemStatus = z.infer<typeof ProblemStatus>;

// Custom Zod transformer for HTML content that sanitizes and validates
const htmlContent = z
  .string()
  .max(VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX)
  .transform(html => sanitizeHtmlContent(html))
  .optional();

export const AssetSchema = z.object({
  path: z.string(),
  kind: z.enum(['image', 'pdf']).optional(),
});
export type Asset = z.infer<typeof AssetSchema>;

// =====================================================
// Answer Configuration Schemas
// =====================================================

export const MCQChoiceSchema = z.object({
  id: z.string().min(1).max(10),
  text: z
    .string()
    .max(ANSWER_CONFIG_CONSTANTS.MCQ.MAX_CHOICE_TEXT_LENGTH)
    .default(''),
});

export type MCQChoice = z.infer<typeof MCQChoiceSchema>;

export const MCQAnswerConfigSchema = z
  .object({
    type: z.literal('mcq'),
    choices: z
      .array(MCQChoiceSchema)
      .min(ANSWER_CONFIG_CONSTANTS.MCQ.MIN_CHOICES)
      .max(ANSWER_CONFIG_CONSTANTS.MCQ.MAX_CHOICES),
    correct_choice_id: z.string().min(1),
    randomize_choices: z.boolean().optional().default(true),
  })
  .refine(data => data.choices.some(c => c.id === data.correct_choice_id), {
    message: 'correct_choice_id must match one of the choice IDs',
  });

export type MCQAnswerConfig = z.infer<typeof MCQAnswerConfigSchema>;

export const ShortAnswerTextConfigSchema = z.object({
  type: z.literal('short'),
  mode: z.literal('text'),
  acceptable_answers: z
    .array(
      z
        .string()
        .min(1)
        .max(ANSWER_CONFIG_CONSTANTS.SHORT_ANSWER.MAX_ANSWER_LENGTH)
    )
    .min(1)
    .max(ANSWER_CONFIG_CONSTANTS.SHORT_ANSWER.MAX_ACCEPTABLE_ANSWERS),
});

export type ShortAnswerTextConfig = z.infer<typeof ShortAnswerTextConfigSchema>;

export const ShortAnswerNumericConfigSchema = z.object({
  type: z.literal('short'),
  mode: z.literal('numeric'),
  numeric_config: z.object({
    correct_value: z.number(),
    tolerance: z
      .number()
      .min(ANSWER_CONFIG_CONSTANTS.SHORT_ANSWER.NUMERIC.MIN_TOLERANCE),
    unit: z
      .string()
      .max(ANSWER_CONFIG_CONSTANTS.SHORT_ANSWER.NUMERIC.MAX_UNIT_LENGTH)
      .optional(),
  }),
});

export type ShortAnswerNumericConfig = z.infer<
  typeof ShortAnswerNumericConfigSchema
>;

export const AnswerConfigSchema = z.union([
  MCQAnswerConfigSchema,
  ShortAnswerTextConfigSchema,
  ShortAnswerNumericConfigSchema,
]);
export type AnswerConfig = z.infer<typeof AnswerConfigSchema>;

export const CreateProblemDto = z.object({
  id: z.uuid().optional(), // Allow client-provided UUID for direct upload approach
  subject_id: z.uuid(),
  title: z
    .string()
    .min(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MIN)
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MAX),
  content: htmlContent,
  problem_type: ProblemType,
  correct_answer: z
    .string()
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX)
    .optional(),
  answer_config: AnswerConfigSchema.nullable().optional(),
  auto_mark: z.boolean().default(false),
  status: ProblemStatus.default(PROBLEM_CONSTANTS.STATUS.NEEDS_REVIEW),
  assets: z.array(AssetSchema).default([]),

  // NEW:
  solution_text: htmlContent,
  solution_assets: z.array(AssetSchema).default([]),
  last_reviewed_date: z.string().optional(),

  tag_ids: z.array(z.uuid()).optional(),
});

export const UpdateProblemDto = z.object({
  id: z.uuid().optional(),
  subject_id: z.uuid().optional(),
  title: z
    .string()
    .min(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MIN)
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MAX)
    .optional(),
  content: htmlContent,
  problem_type: ProblemType.optional(),
  correct_answer: z
    .string()
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TEXT_BODY_MAX)
    .optional(),
  answer_config: AnswerConfigSchema.nullable().optional(),
  auto_mark: z.boolean().optional(),
  status: ProblemStatus.optional(),
  assets: z.array(AssetSchema).nullable().optional(),
  solution_text: htmlContent,
  solution_assets: z.array(AssetSchema).nullable().optional(),
  last_reviewed_date: z.string().optional(),
  tag_ids: z.array(z.uuid()).nullable().optional(),
});

export const CreateTagDto = z.object({
  subject_id: z.uuid(),
  name: z
    .string()
    .min(VALIDATION_CONSTANTS.STRING_LIMITS.TAG_NAME_MIN)
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TAG_NAME_MAX),
});

export const UpdateTagDto = z.object({
  name: z
    .string()
    .min(VALIDATION_CONSTANTS.STRING_LIMITS.TAG_NAME_MIN)
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TAG_NAME_MAX)
    .optional(),
});

export const CreateAttemptDto = z.object({
  problem_id: z.uuid(),
  submitted_answer: z.union([
    z.string().max(ATTEMPT_CONSTANTS.MAX_RESPONSE_LENGTH),
    z.number(),
    z.boolean(),
    z.record(z.string(), z.unknown()),
  ]),
  is_correct: z.boolean().nullable().optional(), // optional for manual types
  cause: z.string().max(ATTEMPT_CONSTANTS.MAX_CAUSE_LENGTH).optional(),
  is_self_assessed: z.boolean().default(false),
  confidence: z.number().int().min(1).max(5).nullable().optional(),
  reflection_notes: z
    .string()
    .max(ATTEMPT_CONSTANTS.MAX_REFLECTION_NOTES_LENGTH)
    .optional(),
  selected_status: ProblemStatus.optional(),
});

export const UpdateAttemptDto = z.object({
  confidence: z.number().int().min(1).max(5).nullable().optional(),
  cause: z
    .string()
    .max(ATTEMPT_CONSTANTS.MAX_CAUSE_LENGTH)
    .nullable()
    .optional(),
  reflection_notes: z
    .string()
    .max(ATTEMPT_CONSTANTS.MAX_REFLECTION_NOTES_LENGTH)
    .nullable()
    .optional(),
  selected_status: ProblemStatus.nullable().optional(),
  submitted_answer: z
    .union([
      z.string().max(ATTEMPT_CONSTANTS.MAX_RESPONSE_LENGTH),
      z.number(),
      z.boolean(),
      z.record(z.string(), z.unknown()),
    ])
    .optional(),
});

// =====================================================
// User Management Types
// =====================================================

// User roles enum
export const UserRole = z.enum([
  USER_ROLES.USER,
  USER_ROLES.MODERATOR,
  USER_ROLES.ADMIN,
  USER_ROLES.SUPER_ADMIN,
]);
export type UserRoleType = z.infer<typeof UserRole>;

// Gender enum
export const Gender = z.enum([
  GENDER_OPTIONS.MALE,
  GENDER_OPTIONS.FEMALE,
  GENDER_OPTIONS.OTHER,
  GENDER_OPTIONS.PREFER_NOT_TO_SAY,
]);
export type GenderType = z.infer<typeof Gender>;

// User profile schema
export const UserProfile = z.object({
  id: z.uuid(),
  username: z.string().nullable(),
  first_name: z.string().nullable(),
  last_name: z.string().nullable(),
  date_of_birth: z.string().date().nullable(),
  gender: Gender.nullable(),
  region: z.string().nullable(),
  timezone: z
    .string()
    .refine(isValidTimezone, { message: 'Invalid IANA timezone' })
    .default('UTC'),
  avatar_url: z.url().nullable(),
  bio: z.string().nullable(),
  user_role: UserRole.default('user'),
  is_active: z.boolean().default(true),
  last_login_at: z.iso.datetime().nullable(),
  created_at: z.iso.datetime(),
  updated_at: z.iso.datetime(),
});

export type UserProfileType = z.infer<typeof UserProfile>;

// User activity log schema
export const UserActivityLog = z.object({
  id: z.uuid(),
  user_id: z.uuid(),
  action: z.string(),
  resource_type: z.string().nullable(),
  resource_id: z.uuid().nullable(),
  details: z.record(z.string(), z.unknown()).nullable(),
  ip_address: z.string().nullable(),
  user_agent: z.string().nullable(),
  created_at: z.iso.datetime(),
});

export type UserActivityLogType = z.infer<typeof UserActivityLog>;

// Admin settings schema
export const AdminSettings = z.object({
  id: z.uuid(),
  key: z.string(),
  value: z.record(z.string(), z.unknown()),
  description: z.string().nullable(),
  updated_by: z.uuid().nullable(),
  created_at: z.iso.datetime(),
  updated_at: z.iso.datetime(),
});

export type AdminSettingsType = z.infer<typeof AdminSettings>;

// User statistics schema
export const UserStatistics = z.object({
  total_users: z.number(),
  active_users: z.number(),
  admin_users: z.number(),
  new_users_today: z.number(),
  new_users_this_week: z.number(),
});

export type UserStatisticsType = z.infer<typeof UserStatistics>;

// Update DTO for user profiles
export const UpdateUserProfileDto = z.object({
  username: z
    .string()
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.USERNAME_MAX)
    .nullable()
    .optional(),
  first_name: z
    .string()
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.FIRST_NAME_MAX)
    .nullable()
    .optional(),
  last_name: z
    .string()
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.LAST_NAME_MAX)
    .nullable()
    .optional(),
  date_of_birth: z
    .union([z.iso.date(), z.literal('')])
    .nullable()
    .optional()
    .transform(val => (val === '' ? null : val)),
  gender: z
    .union([Gender, z.literal('')])
    .nullable()
    .optional()
    .transform(val => (val === '' ? null : val)),
  region: z
    .string()
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.REGION_MAX)
    .nullable()
    .optional(),
  timezone: z
    .string()
    .refine(isValidTimezone, { message: 'Invalid IANA timezone' })
    .optional(),
  avatar_url: z
    .union([z.url(), z.literal('')])
    .nullable()
    .optional()
    .transform(val => (val === '' ? null : val)),
  bio: z
    .string()
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.BIO_MAX)
    .nullable()
    .optional(),
  user_role: UserRole.optional(),
  is_active: z.boolean().optional(),
});

export const UpdateAdminSettingsDto = z.object({
  value: z.record(z.string(), z.unknown()),
  description: z.string().optional(),
});

// Extended user type that includes auth data
export interface ExtendedUser {
  id: string;
  email: string;
  profile: UserProfileType | null;
  isAdmin: boolean;
  isSuperAdmin: boolean;
}

// Admin dashboard data type
export interface AdminDashboardData {
  statistics: UserStatisticsType;
  recentUsers: UserProfileType[];
  recentActivity: UserActivityLogType[];
  systemSettings: AdminSettingsType[];
}

// Subject DTOs
export const CreateSubjectDto = z.object({
  name: z
    .string()
    .min(VALIDATION_CONSTANTS.STRING_LIMITS.SUBJECT_NAME_MIN)
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.SUBJECT_NAME_MAX),
  color: z.enum(SUBJECT_CONSTANTS.COLORS).optional(),
  icon: z.enum(SUBJECT_CONSTANTS.ICONS).optional(),
});

export const UpdateSubjectDto = CreateSubjectDto.partial();

// =====================================================
// Problem Set Schemas
// =====================================================

export const ProblemSetSharingLevel = z.enum([
  PROBLEM_SET_CONSTANTS.SHARING_LEVELS.PRIVATE,
  PROBLEM_SET_CONSTANTS.SHARING_LEVELS.LIMITED,
  PROBLEM_SET_CONSTANTS.SHARING_LEVELS.PUBLIC,
]);
export type ProblemSetSharingLevel = z.infer<typeof ProblemSetSharingLevel>;

export const FilterConfigSchema = z.object({
  tag_ids: z.array(z.uuid()).default([]),
  statuses: z.array(ProblemStatus).default([]),
  problem_types: z.array(ProblemType).default([]),
  days_since_review: z.number().min(0).nullable().optional(),
  include_never_reviewed: z.boolean().default(true),
});
export type FilterConfig = z.infer<typeof FilterConfigSchema>;

export const SessionConfigSchema = z.object({
  randomize: z.boolean().default(true),
  session_size: z.number().min(1).max(100).nullable().optional(),
  auto_advance: z.boolean().default(false),
});
export type SessionConfig = z.infer<typeof SessionConfigSchema>;

export const StartSpacedSessionDto = z.object({
  subject_id: z.uuid(),
  session_size: z
    .number()
    .int()
    .min(1)
    .max(SPACED_REPETITION_CONSTANTS.MAX_SESSION_SIZE)
    .optional(),
});

export const CreateProblemSetDto = z.object({
  subject_id: z.uuid(),
  name: z
    .string()
    .min(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MIN)
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MAX),
  description: htmlContent,
  sharing_level: ProblemSetSharingLevel.default(
    PROBLEM_SET_CONSTANTS.SHARING_LEVELS.PRIVATE
  ),
  shared_with_emails: z.array(z.email()).optional(),
  problem_ids: z.array(z.uuid()).optional(),
  is_smart: z.boolean().default(false),
  filter_config: FilterConfigSchema.nullable().optional(),
  session_config: SessionConfigSchema.nullable().optional(),
  allow_copying: z.boolean().default(true),
});

export const UpdateProblemSetDto = z.object({
  name: z
    .string()
    .min(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MIN)
    .max(VALIDATION_CONSTANTS.STRING_LIMITS.TITLE_MAX)
    .optional(),
  description: htmlContent.nullable(),
  sharing_level: ProblemSetSharingLevel.optional(),
  shared_with_emails: z.array(z.email()).nullable().optional(),
  problem_ids: z.array(z.uuid()).optional(),
  is_smart: z.boolean().optional(),
  filter_config: FilterConfigSchema.nullable().optional(),
  session_config: SessionConfigSchema.nullable().optional(),
  allow_copying: z.boolean().optional(),
  is_listed: z.boolean().optional(),
  discovery_subject: z
    .string()
    .nullable()
    .optional()
    .refine(
      val =>
        val === undefined ||
        val === null ||
        val === '' ||
        (DISCOVERY_SUBJECTS as readonly string[]).includes(val),
      { message: 'Invalid discovery subject' }
    )
    .transform(val => (val === '' ? null : val)),
});

export const AddProblemsToSetDto = z.object({
  problem_ids: z.array(z.uuid()),
});

export const RemoveProblemsFromSetDto = z.object({
  problem_ids: z.array(z.uuid()),
});

// =====================================================
// QR Upload Session Schemas
// =====================================================

// =====================================================
// Error Categorisation & Insights Schemas
// =====================================================

const ErrorBroadCategorySchema = z.enum(ERROR_CATEGORY_VALUES);

export const UpdateErrorCategorisationDto = z.object({
  broad_category: ErrorBroadCategorySchema.optional(),
  granular_tag: z.string().min(1).max(200).optional(),
});

export const StartInsightsReviewDto = z.object({
  subject_id: z.uuid(),
  problem_ids: z.array(z.uuid()).min(1).max(50),
});

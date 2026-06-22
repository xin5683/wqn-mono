import ProblemsPageClient from './problems-page-client';
import { ROUTES } from '@/lib/constants';
import { BackLink } from '@/components/back-link';
import { Tag } from '@/lib/types';
import { getTranslations } from 'next-intl/server';
import { redirect } from 'next/navigation';
import { getCurrentUser, serverApi } from '@/lib/api/server';
import { ApiError } from '@/lib/api/errors';
import {
  ProblemListResponseSchema,
  SubjectResponseSchema,
} from '@/lib/api/response-schemas';

export async function generateMetadata({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const subject = await loadSubject(id);
  const tMeta = await getTranslations('Metadata');
  return {
    title: tMeta('subjectProblemsMetaTitle', { subject: subject?.name ?? '' }),
  };
}

// metadata 上下文不能 redirect/notFound，失败时降级返回 null。
async function loadSubject(subjectId: string) {
  try {
    return await serverApi(
      `/api/subjects/${subjectId}`,
      {},
      SubjectResponseSchema
    );
  } catch (err) {
    console.error('[problems] loadSubject failed', { subjectId, err });
    return null;
  }
}

// subject 查询失败（含 404）降级为 null，由页面走 notFound；
// problems / tags 查询失败则向上抛出，由页面渲染加载失败错误态
// （不再静默返回空列表，那样会把真实错误伪装成“数据消失”）。
async function loadProblemsPage(subjectId: string) {
  const [subject, problems, availableTags] = await Promise.all([
    serverApi(`/api/subjects/${subjectId}`, {}, SubjectResponseSchema).catch(
      err => {
        console.error('[problems] loadSubject failed', { subjectId, err });
        return null;
      }
    ),
    serverApi(
      `/api/problems?subject_id=${subjectId}`,
      {},
      ProblemListResponseSchema
    ),
    serverApi<Tag[]>(`/api/tags?subject_id=${subjectId}`),
  ]);
  return {
    subject,
    problems: problems || [],
    availableTags: availableTags || [],
  };
}

export default async function SubjectProblemsPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const user = await getCurrentUser();
  if (!user) {
    redirect(`/auth/login?redirect=/subjects/${id}/problems`);
  }

  const t = await getTranslations('Subjects');
  const tCommon = await getTranslations('Common');

  const result = await loadProblemsPage(id).then(
    data => ({ ok: true as const, data }),
    err => {
      const requestId = err instanceof ApiError ? err.requestId : null;
      console.error('[problems] loadProblemsPage failed', {
        subjectId: id,
        requestId,
        message: err instanceof Error ? err.message : String(err),
      });
      return {
        ok: false as const,
        requestId,
        message: err instanceof Error ? err.message : String(err),
      };
    }
  );

  if (!result.ok) {
    return (
      <div className="section-container">
        <div className="page-header">
          <h1 className="page-title">{tCommon('error')}</h1>
          <p className="page-description">{t('loadFailed')}</p>
          {result.requestId && (
            <p className="text-body-sm text-muted-foreground">
              Request ID: <code className="font-mono">{result.requestId}</code>
            </p>
          )}
        </div>
        <BackLink href={ROUTES.SUBJECTS}>{tCommon('refresh')}</BackLink>
      </div>
    );
  }

  const { subject, problems, availableTags } = result.data;

  if (!subject) {
    return (
      <div className="section-container">
        <p className="text-body-sm text-muted-foreground">{t('notFound')}</p>
        <BackLink href={ROUTES.SUBJECTS}>{t('backToShelf')}</BackLink>
      </div>
    );
  }

  return (
    <div className="section-container">
      <div className="flex items-center justify-between">
        <div className="page-header">
          <h1 className="page-title">
            {subject.name} — {t('problems')}
          </h1>
          <p className="page-description">{t('pageDescription')}</p>
        </div>
        <BackLink href={ROUTES.SUBJECTS}>
          <span className="hidden md:inline">{t('backToShelf')}</span>
          <span className="md:hidden">{tCommon('back')}</span>
        </BackLink>
      </div>

      {/* Problems page with create form and search */}
      <ProblemsPageClient
        initialProblems={problems}
        subjectId={subject.id}
        availableTags={availableTags}
      />
    </div>
  );
}

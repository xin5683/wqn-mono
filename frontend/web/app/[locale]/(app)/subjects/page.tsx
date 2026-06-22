import type { Metadata } from 'next';
import { getTranslations } from 'next-intl/server';
import SubjectsPageClient from './subjects-page-client';
import { serverApi } from '@/lib/api/server';
import { SubjectWithMetadata } from '@/lib/types';
import { SubjectListResponseSchema } from '@/lib/api/response-schemas';

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations('Metadata');
  return { title: t('shelfMetaTitle') };
}

async function loadSubjects() {
  try {
    const subjects = await serverApi(
      '/api/subjects',
      {},
      SubjectListResponseSchema
    );
    return { data: subjects || [] };
  } catch {
    return { data: [] as SubjectWithMetadata[] };
  }
}

export default async function SubjectsPage() {
  const { data } = await loadSubjects();

  return <SubjectsPageClient initialSubjects={data} />;
}

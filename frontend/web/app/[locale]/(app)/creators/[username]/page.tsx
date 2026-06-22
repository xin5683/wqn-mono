import { notFound } from 'next/navigation';
import { getTranslations } from 'next-intl/server';
import CreatorProfileClient from './creator-profile-client';
import type { Metadata } from 'next';
import { serverApi } from '@/lib/api/server';
import type { CreatorResponse } from '@/lib/types';
import { absoluteSiteUrl } from '@/lib/api/url';

export async function generateMetadata({
  params,
}: {
  params: Promise<{ username: string }>;
}): Promise<Metadata> {
  const { username } = await params;
  const creator = await serverApi<CreatorResponse>(
    `/api/creators/${encodeURIComponent(username)}`
  ).catch(() => null);
  const profile = creator?.profile;

  const t = await getTranslations('Creator');
  const tMeta = await getTranslations('Metadata');
  if (!profile) return { title: t('notFound') };

  const displayName = profile.display_name || username;
  const description = profile.bio
    ? profile.bio.substring(0, 160)
    : t('problemSetsBy', { name: displayName });

  const creatorTitle = tMeta('creatorMetaTitle', { username });
  const siteName = tMeta('siteName');

  return {
    title: creatorTitle,
    description,
    openGraph: {
      title: `${creatorTitle} – ${siteName}`,
      description,
      url: absoluteSiteUrl(`/creators/${username}`),
      siteName,
    },
    alternates: {
      canonical: absoluteSiteUrl(`/creators/${username}`),
    },
  };
}

export default async function CreatorProfilePage({
  params,
}: {
  params: Promise<{ username: string }>;
}) {
  const { username } = await params;
  const creator = await serverApi<CreatorResponse>(
    `/api/creators/${encodeURIComponent(username)}`
  ).catch(() => null);

  if (!creator?.profile) notFound();

  return (
    <CreatorProfileClient
      profile={creator.profile}
      sets={creator.sets || []}
      aggregateStats={creator.aggregateStats}
    />
  );
}

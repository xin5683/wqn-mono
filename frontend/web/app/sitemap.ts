import type { MetadataRoute } from 'next';
import { absoluteSiteUrl } from '@/lib/api/url';

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  return [
    {
      url: absoluteSiteUrl('/'),
      changeFrequency: 'monthly' as const,
      priority: 1.0,
    },
    {
      url: absoluteSiteUrl('/discover'),
      changeFrequency: 'daily' as const,
      priority: 0.9,
    },
    {
      url: absoluteSiteUrl('/privacy'),
      changeFrequency: 'monthly' as const,
      priority: 0.3,
    },
  ];
}

import type { MetadataRoute } from 'next';
import { absoluteSiteUrl, getCanonicalSiteUrl } from '@/lib/api/url';

export default function robots(): MetadataRoute.Robots {
  return {
    rules: {
      userAgent: '*',
      allow: '/',
      disallow: ['/admin/', '/api/', '/auth/'],
    },
    host: getCanonicalSiteUrl(),
    sitemap: absoluteSiteUrl('/sitemap.xml'),
  };
}

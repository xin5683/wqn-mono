const publicPathPrefixes = [
  '/auth',
  '/privacy',
  '/upload',
  '/discover',
  '/creators',
  '/problem-sets',
];

export function isPublicContentPath(contentPath: string): boolean {
  if (contentPath === '/') {
    return true;
  }

  return publicPathPrefixes.some(
    prefix => contentPath === prefix || contentPath.startsWith(`${prefix}/`)
  );
}

import { getCurrentUser } from '@/lib/api/server';
import { ProfileSheet } from '@/components/profile-sheet';
import { AuthButtons } from '@/components/auth-buttons';

export async function ProfileButton() {
  const user = await getCurrentUser();

  if (!user) {
    return <AuthButtons />;
  }

  return (
    <ProfileSheet
      initialProfile={user.profile ?? null}
      email={user.email ?? ''}
    />
  );
}

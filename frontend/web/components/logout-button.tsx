'use client';

import { Button } from '@/components/ui/button';
import { useRouter } from '@/i18n/navigation';
import { LogOut } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { clientApi } from '@/lib/api/client';

export function LogoutButton() {
  const t = useTranslations('Profile');
  const router = useRouter();

  const logout = async () => {
    await clientApi('/api/auth/logout', {
      method: 'POST',
    });

    router.replace('/');
    router.refresh();
  };

  return (
    <Button
      variant="ghost"
      size="icon"
      className="h-11 w-11"
      onClick={logout}
      aria-label={t('signOut')}
      title={t('signOut')}
    >
      <LogOut className="h-4 w-4" />
    </Button>
  );
}

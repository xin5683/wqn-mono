'use client';

import { useTranslations } from 'next-intl';
import { Link } from '@/i18n/navigation';
import { UserX } from 'lucide-react';
import { Button } from '@/components/ui/button';

export function RegistrationClosedNotice() {
  const t = useTranslations('Auth');
  return (
    <div className="auth-card-orange">
      {/* Icon header */}
      <div className="flex justify-center mb-6 auth-icon-entrance">
        <div className="auth-icon-box-orange">
          <UserX className="w-6 h-6 text-orange-600 dark:text-orange-400" />
        </div>
      </div>

      {/* Title */}
      <div className="text-center mb-6 space-y-2">
        <h1 className="auth-title">{t('registrationClosedTitle')}</h1>
        <p className="auth-subtitle">{t('registrationClosedDesc')}</p>
      </div>

      <Button asChild className="w-full btn-cta-primary">
        <Link href="/auth/login">{t('backToLogin')}</Link>
      </Button>
    </div>
  );
}

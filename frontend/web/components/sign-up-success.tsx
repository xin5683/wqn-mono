'use client';

import { Button } from '@/components/ui/button';
import { Link } from '@/i18n/navigation';
import { CheckCircle } from 'lucide-react';
import { useTranslations } from 'next-intl';

export function SignUpSuccess() {
  const t = useTranslations('Auth');

  return (
    <div className="w-full auth-fade-in">
      <div className="auth-card-green">
        <div className="flex justify-center mb-6 auth-icon-entrance">
          <div className="auth-icon-box-green">
            <CheckCircle className="w-6 h-6 text-green-600 dark:text-green-400" />
          </div>
        </div>

        <div className="text-center mb-6 space-y-2">
          <h1 className="auth-title">{t('accountCreated')}</h1>
          <p className="auth-subtitle">{t('startYourJourney')}</p>
        </div>

        <Button asChild className="w-full btn-cta-primary">
          <Link href="/subjects">{t('continueToApp')}</Link>
        </Button>
      </div>
    </div>
  );
}

'use client';

import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Link } from '@/i18n/navigation';
import { useRouter } from '@/i18n/navigation';
import { useState } from 'react';
import { ERROR_MESSAGES } from '@/lib/constants';
import { UserPlus, Eye, EyeOff } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { validatePayload } from '@/lib/validation/payload';
import { UpdateUserProfileDto } from '@/lib/validation/schemas';
import { clientApi, ClientApiError } from '@/lib/api/client';

export function SignUpForm({
  className,
  ...props
}: React.ComponentPropsWithoutRef<'div'>) {
  const t = useTranslations('Auth');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [repeatPassword, setRepeatPassword] = useState('');
  const [agreedToPrivacy, setAgreedToPrivacy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [showRepeatPassword, setShowRepeatPassword] = useState(false);
  const router = useRouter();

  const handleSignUp = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);

    if (!agreedToPrivacy) {
      setError(t('privacyPolicyRequired'));
      setIsLoading(false);
      return;
    }

    if (password !== repeatPassword) {
      setError(t('passwordsDoNotMatch'));
      setIsLoading(false);
      return;
    }

    try {
      const detectedTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
      await clientApi('/api/auth/sign-up', {
        method: 'POST',
        body: { email, password, timezone: detectedTz },
      });

      if (detectedTz) {
        try {
          const profilePayload = validatePayload(
            { timezone: detectedTz },
            UpdateUserProfileDto,
            'update profile'
          );
          clientApi('/api/profile', {
            method: 'PATCH',
            body: profilePayload,
          }).catch(() => {});
        } catch {
          // Keep the best-effort profile sync from blocking successful sign-up.
        }
      }

      router.push(`/auth/sign-up-success?email=${encodeURIComponent(email)}`);
      router.refresh();
    } catch (error: unknown) {
      // 403 means registration was closed between page load and submit
      // (a super-admin flipped the toggle). Show the localized notice
      // rather than the raw "Forbidden" message.
      if (error instanceof ClientApiError && error.status === 403) {
        setError(t('registrationClosedDesc'));
      } else {
        setError(
          error instanceof Error ? error.message : ERROR_MESSAGES.INTERNAL_ERROR
        );
      }
      setIsLoading(false);
    }
  };

  return (
    <div className={cn('w-full auth-fade-in', className)} {...props}>
      <div className="auth-card-orange">
        {/* Icon header */}
        <div className="flex justify-center mb-6 auth-icon-entrance">
          <div className="auth-icon-box-orange">
            <UserPlus className="w-6 h-6 text-orange-600 dark:text-orange-400" />
          </div>
        </div>

        {/* Title */}
        <div className="text-center mb-6 space-y-2">
          <h1 className="auth-title">{t('createYourNotebook')}</h1>
          <p className="auth-subtitle">{t('startYourJourney')}</p>
        </div>

        {/* Form */}
        <form onSubmit={handleSignUp} className="auth-slide-up space-y-4">
          <div className="space-y-2">
            <Label htmlFor="email">{t('email')}</Label>
            <Input
              id="email"
              type="email"
              placeholder={t('emailPlaceholder')}
              required
              value={email}
              onChange={e => setEmail(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="password">{t('password')}</Label>
            <div className="relative">
              <Input
                id="password"
                type={showPassword ? 'text' : 'password'}
                required
                value={password}
                onChange={e => setPassword(e.target.value)}
                className="pr-10"
              />
              <button
                type="button"
                onClick={() => setShowPassword(prev => !prev)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                aria-label={
                  showPassword ? t('hidePassword') : t('showPassword')
                }
              >
                {showPassword ? (
                  <EyeOff className="h-4 w-4" />
                ) : (
                  <Eye className="h-4 w-4" />
                )}
              </button>
            </div>
          </div>
          <div className="space-y-2">
            <Label htmlFor="repeat-password">{t('repeatPassword')}</Label>
            <div className="relative">
              <Input
                id="repeat-password"
                type={showRepeatPassword ? 'text' : 'password'}
                required
                value={repeatPassword}
                onChange={e => setRepeatPassword(e.target.value)}
                className="pr-10"
              />
              <button
                type="button"
                onClick={() => setShowRepeatPassword(prev => !prev)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                aria-label={
                  showRepeatPassword ? t('hidePassword') : t('showPassword')
                }
              >
                {showRepeatPassword ? (
                  <EyeOff className="h-4 w-4" />
                ) : (
                  <Eye className="h-4 w-4" />
                )}
              </button>
            </div>
          </div>
          <div className="flex items-start gap-2">
            <input
              id="privacy-policy"
              type="checkbox"
              checked={agreedToPrivacy}
              onChange={e => setAgreedToPrivacy(e.target.checked)}
              className="mt-0.5 h-4 w-4 shrink-0 rounded border-gray-300 accent-orange-500"
            />
            <label
              htmlFor="privacy-policy"
              className="text-sm text-gray-600 dark:text-gray-400"
            >
              {t('privacyPolicyAgreement')}{' '}
              <Link href="/privacy" className="auth-link underline">
                {t('privacyPolicy')}
              </Link>
            </label>
          </div>
          {error && <p className="form-error">{error}</p>}
          <Button
            type="submit"
            className="w-full btn-cta-primary"
            disabled={isLoading}
          >
            {isLoading ? t('creatingAccount') : t('signUp')}
          </Button>
        </form>

        {/* Links */}
        <div className="mt-6 text-center text-sm text-gray-600 dark:text-gray-400">
          {t('alreadyHaveAccount')}{' '}
          <Link href="/auth/login" className="auth-link underline">
            {t('login')}
          </Link>
        </div>
      </div>
    </div>
  );
}

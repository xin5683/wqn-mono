import { SignUpForm } from '@/components/sign-up-form';
import { RegistrationClosedNotice } from '@/components/registration-closed-notice';
import { getCurrentUser, serverApi } from '@/lib/api/server';
import { RegistrationStatusResponseSchema } from '@/lib/api/response-schemas';
import { redirect } from 'next/navigation';

export default async function SignUpPage() {
  const user = await getCurrentUser();

  if (user) {
    redirect('/subjects');
  }

  // If a super-admin has closed self-service registration, replace the
  // form with a notice instead of letting users fill it out and fail.
  // A fetch failure defaults to open so sign-up is never blocked by a
  // transient backend hiccup.
  const status = await serverApi(
    '/api/registration-status',
    {},
    RegistrationStatusResponseSchema
  ).catch(() => ({ enabled: true }));

  if (!status.enabled) {
    return (
      <div className="auth-page-container">
        <div className="auth-form-wrapper">
          <RegistrationClosedNotice />
        </div>
      </div>
    );
  }

  return (
    <div className="auth-page-container">
      <div className="auth-form-wrapper">
        <SignUpForm />
      </div>
    </div>
  );
}

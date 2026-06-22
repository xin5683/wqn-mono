import { ForgotPasswordForm } from '@/components/forgot-password-form';
import { getCurrentUser } from '@/lib/api/server';
import { redirect } from 'next/navigation';

export default async function ForgotPasswordPage() {
  const user = await getCurrentUser();

  if (user) {
    redirect('/subjects');
  }

  return (
    <div className="auth-page-container">
      <div className="auth-form-wrapper">
        <ForgotPasswordForm />
      </div>
    </div>
  );
}

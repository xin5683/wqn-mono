import { UpdatePasswordForm } from '@/components/update-password-form';
import { getCurrentUser } from '@/lib/api/server';
import { ROUTES } from '@/lib/constants';
import { redirect } from 'next/navigation';

export default async function UpdatePasswordPage() {
  const user = await getCurrentUser();

  if (!user) {
    redirect(ROUTES.HOME);
  }

  return (
    <div className="auth-page-container">
      <div className="auth-form-wrapper">
        <UpdatePasswordForm />
      </div>
    </div>
  );
}

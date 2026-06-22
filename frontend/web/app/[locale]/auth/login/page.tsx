import { LoginForm } from '@/components/login-form';
import { getCurrentUser } from '@/lib/api/server';
import { redirect } from 'next/navigation';
import { ROUTES } from '@/lib/constants';

interface PageProps {
  searchParams: Promise<{ redirect?: string }>;
}

export default async function LoginPage({ searchParams }: PageProps) {
  const user = await getCurrentUser();
  const { redirect: redirectParam } = await searchParams;

  if (user) {
    const destination = redirectParam || ROUTES.SUBJECTS;
    redirect(destination);
  }

  return (
    <div className="auth-page-container">
      <div className="auth-form-wrapper">
        <LoginForm redirectTo={redirectParam} />
      </div>
    </div>
  );
}

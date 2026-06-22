import { redirect } from 'next/navigation';
import { getCurrentUser } from '@/lib/api/server';
import { AdminLayoutShell } from '@/components/admin/admin-layout-shell';
import { ROUTES } from '@/lib/constants';
import { getTranslations } from 'next-intl/server';
import type { Metadata } from 'next';

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations('Metadata');
  return { title: t('adminMetaTitle') };
}

export default async function AdminLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const user = await getCurrentUser();
  if (!user) {
    redirect(ROUTES.AUTH.LOGIN);
  }

  if (user.role !== 'super_admin') {
    redirect(ROUTES.SUBJECTS);
  }

  return <AdminLayoutShell>{children}</AdminLayoutShell>;
}

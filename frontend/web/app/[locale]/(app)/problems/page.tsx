import { PageHeader } from '@/components/page-header';
import { Card, CardContent } from '@/components/ui/card';
import { serverApi } from '@/lib/api/server';
import { Link } from '@/i18n/navigation';
import { SubjectListResponseSchema } from '@/lib/api/response-schemas';

export default async function ProblemsChooser() {
  const subjects = await serverApi(
    '/api/subjects',
    {},
    SubjectListResponseSchema
  ).catch(() => []);

  return (
    <div className="section-container">
      <PageHeader
        title="Problems"
        description="Pick a subject to browse and review problems."
      />

      <Card className="card-section">
        <CardContent className="card-section-content pt-6">
          <ul className="space-y-2">
            {(subjects ?? []).map(s => (
              <li key={s.id}>
                <Link
                  className="text-primary underline hover:text-primary/80 transition-colors"
                  href={`/subjects/${s.id}/problems`}
                >
                  {s.name}
                </Link>
              </li>
            ))}
          </ul>

          <div className="mt-6 border-t pt-4">
            <p className="text-body-sm text-muted-foreground">
              Tip: add a subject first on the{' '}
              <Link
                href="/subjects"
                className="underline text-primary hover:text-primary/80 transition-colors"
              >
                Subjects
              </Link>{' '}
              page.
            </p>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

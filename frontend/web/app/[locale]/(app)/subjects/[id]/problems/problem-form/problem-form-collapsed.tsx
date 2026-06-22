'use client';

import { PenLine, ScanLine } from 'lucide-react';
import { useTranslations } from 'next-intl';
import { Button } from '@/components/ui/button';
import {
  ImageScanUploader,
  type ExtractionQuota,
  type ImageAttachment,
} from '@/components/ui/image-scan-uploader';
import type { ExtractedProblemData } from '@/lib/types';

interface ProblemFormCollapsedProps {
  subjectId: string;
  showImageScan: boolean;
  extractionQuota: ExtractionQuota | null;
  setExtractionQuota: (quota: ExtractionQuota | null) => void;
  onExtracted: (
    data: ExtractedProblemData,
    imageAttachment?: ImageAttachment
  ) => void;
  onManual: () => void;
  onScan: () => void;
  onCancelScan: () => void;
}

export function ProblemFormCollapsed({
  subjectId,
  showImageScan,
  extractionQuota,
  setExtractionQuota,
  onExtracted,
  onManual,
  onScan,
  onCancelScan,
}: ProblemFormCollapsedProps) {
  const t = useTranslations('Subjects');

  return (
    <div className="space-y-3">
      {!showImageScan && (
        <div className="grid grid-cols-2 gap-3">
          <Button
            type="button"
            variant="outline"
            onClick={onManual}
            className="border-dashed text-muted-foreground hover:border-amber-400/50 dark:hover:border-amber-500/50 hover:bg-amber-50/50 dark:hover:bg-amber-950/20 hover:text-amber-900 dark:hover:text-amber-100 justify-center transition-colors py-6"
          >
            <PenLine className="h-4 w-4 mr-2" />
            {t('writeManually')}
          </Button>
          <Button
            type="button"
            variant="outline"
            onClick={onScan}
            className="border-dashed text-muted-foreground hover:border-blue-400/50 dark:hover:border-blue-500/50 hover:bg-blue-50/50 dark:hover:bg-blue-950/20 hover:text-blue-900 dark:hover:text-blue-100 justify-center transition-colors py-6"
          >
            <div className="flex items-center">
              <ScanLine className="h-4 w-4 mr-2" />
              {t('scanFromImage')}
            </div>
          </Button>
        </div>
      )}
      {showImageScan && (
        <ImageScanUploader
          subjectId={subjectId}
          onExtracted={onExtracted}
          onCancel={onCancelScan}
          quota={extractionQuota}
          onQuotaChange={setExtractionQuota}
        />
      )}
    </div>
  );
}

'use client';

import {
  ImageScanUploader,
  type ExtractionQuota,
  type ImageAttachment,
} from '@/components/ui/image-scan-uploader';
import type { ExtractedProblemData } from '@/lib/types';

interface ProblemFormScanProps {
  subjectId: string;
  extractionQuota: ExtractionQuota | null;
  setExtractionQuota: (quota: ExtractionQuota | null) => void;
  onExtracted: (
    data: ExtractedProblemData,
    imageAttachment?: ImageAttachment
  ) => void;
  onCancel: () => void;
}

export function ProblemFormScan({
  subjectId,
  extractionQuota,
  setExtractionQuota,
  onExtracted,
  onCancel,
}: ProblemFormScanProps) {
  return (
    <div className="space-y-3">
      <ImageScanUploader
        subjectId={subjectId}
        onExtracted={onExtracted}
        onCancel={onCancel}
        quota={extractionQuota}
        onQuotaChange={setExtractionQuota}
      />
    </div>
  );
}

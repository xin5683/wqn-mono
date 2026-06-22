'use client';

import { RefObject, useCallback, useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'next-intl';
import { uploadFiles } from '@/lib/storage/client';
import { generateUuid } from '@/lib/utils/uuid';
import type { Problem } from '@/lib/types';
import type { RichTextEditorHandle } from '@/components/editor';

export interface FileAsset {
  path: string;
  name: string;
}

interface PendingImageAttachment {
  file: File;
  roles: ('problem' | 'solution')[];
}

interface UseProblemAssetsOptions {
  problem: Problem | null;
  isEditMode: boolean;
  isExpanded: boolean;
  contentEditorRef: RefObject<RichTextEditorHandle | null>;
  solutionEditorRef: RefObject<RichTextEditorHandle | null>;
}

function assetFromPath(path: string): FileAsset {
  return {
    path,
    name: path.split('/').pop() || '',
  };
}

export function useProblemAssets({
  problem,
  isEditMode,
  isExpanded,
  contentEditorRef,
  solutionEditorRef,
}: UseProblemAssetsOptions) {
  const t = useTranslations('Subjects');
  const tProblems = useTranslations('Problems');
  const [problemAssets, setProblemAssets] = useState<FileAsset[]>(
    problem?.assets?.map((asset: any) => assetFromPath(asset.path)) || []
  );
  const [solutionText, setSolutionText] = useState(
    problem?.solution_text || ''
  );
  const [solutionAssets, setSolutionAssets] = useState<FileAsset[]>(
    problem?.solution_assets?.map((asset: any) => assetFromPath(asset.path)) ||
      []
  );
  const [problemUuid, setProblemUuid] = useState<string | null>(null);
  // 创建成功后置 true，阻止组件卸载时的 cleanup 误删已保存的题目
  // （cleanup 本意只清理“填了表单但没提交就离开”时上传的图片）。
  const savedRef = useRef(false);
  const [pendingImageAttachment, setPendingImageAttachment] =
    useState<PendingImageAttachment | null>(null);

  useEffect(() => {
    if (!isEditMode && isExpanded && !problemUuid) {
      // generateUuid works in non-secure contexts (plain HTTP via LAN IP),
      // unlike crypto.randomUUID which is undefined there.
      setProblemUuid(generateUuid());
    }
  }, [isEditMode, isExpanded, problemUuid]);

  useEffect(() => {
    if (!problemUuid || !pendingImageAttachment) return;

    const { file, roles } = pendingImageAttachment;
    setPendingImageAttachment(null);

    (async () => {
      for (const role of roles) {
        try {
          const paths = await uploadFiles([file], role, problemUuid);
          const newAsset = {
            path: paths[0],
            name: file.name.replace(/\s+/g, '_'),
          };
          if (role === 'problem') {
            setProblemAssets(prev => [...prev, newAsset]);
          } else {
            setSolutionAssets(prev => [...prev, newAsset]);
          }
        } catch (err: any) {
          toast.error(
            tProblems('failedToSaveImageAsset', {
              role,
              error: err.message || '',
            })
          );
        }
      }
    })();
  }, [problemUuid, pendingImageAttachment, tProblems]);

  const handleInsertProblemImage = useCallback(
    (path: string, name: string) => {
      if (!contentEditorRef.current?.editor) {
        toast.error(t('editorNotReady'));
        return;
      }

      const imageUrl = `/api/files/${encodeURIComponent(path)}`;
      contentEditorRef.current.editor
        .chain()
        .focus()
        .setResizableImage({
          src: imageUrl,
          alt: name,
        })
        .run();

      toast.success(t('imageInserted'));
    },
    [contentEditorRef, t]
  );

  const handleInsertSolutionImage = useCallback(
    (path: string, name: string) => {
      if (!solutionEditorRef.current?.editor) {
        toast.error(t('editorNotReady'));
        return;
      }

      const imageUrl = `/api/files/${encodeURIComponent(path)}`;
      solutionEditorRef.current.editor
        .chain()
        .focus()
        .setResizableImage({
          src: imageUrl,
          alt: name,
        })
        .run();

      toast.success(t('solutionImageInserted'));
    },
    [solutionEditorRef, t]
  );

  const cleanupUnsavedProblem = useCallback(
    async (uuidToCleanup: string | null) => {
      if (isEditMode || !uuidToCleanup) return;
      // 题目已成功提交保存，不要在卸载时清理它。
      if (savedRef.current) return;

      try {
        if (navigator.sendBeacon) {
          const formData = new FormData();
          formData.append('problemId', uuidToCleanup);
          navigator.sendBeacon(
            `/api/problems/${uuidToCleanup}/cleanup`,
            formData
          );
        } else {
          await fetch(`/api/problems/${uuidToCleanup}/cleanup`, {
            method: 'DELETE',
            keepalive: true,
          });
        }
      } catch (error) {
        console.warn('Failed to cleanup unsaved problem assets:', error);
      }
    },
    [isEditMode]
  );

  useEffect(() => {
    const handleBeforeUnload = () => {
      cleanupUnsavedProblem(problemUuid);
    };

    window.addEventListener('beforeunload', handleBeforeUnload);

    return () => {
      cleanupUnsavedProblem(problemUuid);
      window.removeEventListener('beforeunload', handleBeforeUnload);
    };
  }, [problemUuid, cleanupUnsavedProblem]);

  function queueImageAttachment(file: File, roles: ('problem' | 'solution')[]) {
    if (roles.length > 0) {
      setPendingImageAttachment({ file, roles });
    }
  }

  function setSolutionHtml(html: string) {
    solutionEditorRef.current?.setContent(html);
    setSolutionText(html);
  }

  function resetAfterCreate() {
    savedRef.current = true;
    setSolutionText('');
    setProblemAssets([]);
    setSolutionAssets([]);
    setProblemUuid(null);
  }

  return {
    problemAssets,
    setProblemAssets,
    solutionText,
    setSolutionText,
    solutionAssets,
    setSolutionAssets,
    problemUuid,
    setProblemUuid,
    queueImageAttachment,
    setSolutionHtml,
    handleInsertProblemImage,
    handleInsertSolutionImage,
    cleanupUnsavedProblem,
    resetAfterCreate,
  };
}

export type ProblemAssetsState = ReturnType<typeof useProblemAssets>;

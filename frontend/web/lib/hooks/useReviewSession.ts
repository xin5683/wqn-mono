'use client';

import { useState, useRef } from 'react';
import { useRouter } from '@/i18n/navigation';
import { toast } from 'sonner';
import { clientApi } from '@/lib/api/client';

interface ResumeDialogState {
  open: boolean;
  session: any;
  problemSetId: string | null;
}

interface StartSessionResult {
  hasActiveSession?: boolean;
  session?: any;
  sessionId: string;
}

export function useReviewSession() {
  const router = useRouter();
  const [sessionLoading, setSessionLoading] = useState<string | null>(null);
  const [resumeDialog, setResumeDialog] = useState<ResumeDialogState>({
    open: false,
    session: null,
    problemSetId: null,
  });

  // Ref to track current fetch abort controller
  const abortControllerRef = useRef<AbortController | null>(null);

  const startReview = async (problemSetId: string) => {
    // Cancel any existing in-flight request
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    const abortController = new AbortController();
    abortControllerRef.current = abortController;

    setSessionLoading(problemSetId);
    try {
      const result = await clientApi<StartSessionResult>(
        `/api/problem-sets/${problemSetId}/start-session`,
        { method: 'POST', signal: abortController.signal }
      );

      if (result.hasActiveSession) {
        setResumeDialog({
          open: true,
          session: result.session,
          problemSetId,
        });
      } else {
        router.push(
          `/problem-sets/${problemSetId}/review?sessionId=${result.sessionId}`
        );
      }
    } catch (error) {
      // Ignore abort errors and "Failed to fetch" which can happen on navigation
      if (error instanceof Error) {
        if (error.name === 'AbortError') {
          return;
        }
        if (error.name === 'TypeError' && error.message === 'Failed to fetch') {
          return;
        }
        toast.error(
          error instanceof Error ? error.message : 'Failed to start review'
        );
      }
    } finally {
      setSessionLoading(null);
      abortControllerRef.current = null;
    }
  };

  const resumeSession = (sessionId: string) => {
    const psId = resumeDialog.problemSetId;
    setResumeDialog({ open: false, session: null, problemSetId: null });
    router.push(`/problem-sets/${psId}/review?sessionId=${sessionId}`);
  };

  const startNewSession = async () => {
    const psId = resumeDialog.problemSetId;
    const oldSessionId = resumeDialog.session?.id;

    // Cancel any existing in-flight request
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    const abortController = new AbortController();
    abortControllerRef.current = abortController;

    setResumeDialog({ open: false, session: null, problemSetId: null });
    setSessionLoading(psId);
    try {
      if (oldSessionId) {
        await clientApi(`/api/review-sessions/${oldSessionId}`, {
          method: 'DELETE',
          signal: abortController.signal,
        });
      }
      const data = await clientApi<StartSessionResult>(
        `/api/problem-sets/${psId}/start-session`,
        {
          method: 'POST',
          signal: abortController.signal,
        }
      );
      router.push(`/problem-sets/${psId}/review?sessionId=${data.sessionId}`);
    } catch (error) {
      // Ignore abort errors and "Failed to fetch" which can happen on navigation
      if (error instanceof Error) {
        if (error.name === 'AbortError') {
          return;
        }
        if (error.name === 'TypeError' && error.message === 'Failed to fetch') {
          return;
        }
      }
      toast.error('Failed to start new session');
    } finally {
      setSessionLoading(null);
      abortControllerRef.current = null;
    }
  };

  const setResumeDialogOpen = (open: boolean) => {
    setResumeDialog(prev => ({ ...prev, open }));
  };

  return {
    sessionLoading,
    resumeDialog,
    startReview,
    resumeSession,
    startNewSession,
    setResumeDialogOpen,
  };
}

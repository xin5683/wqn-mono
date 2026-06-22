'use client';

import { useState, useEffect, useCallback, useMemo } from 'react';
import { useRouter } from '@/i18n/navigation';
import { useTranslations } from 'next-intl';
import { DataTable } from '@/components/problems/data-table';
import { createColumns } from './columns';
import CompactSearchFilter from '@/components/problems/compact-search-filter';
import {
  AddProblemsToSetDto,
  ProblemStatus,
  ProblemType,
} from '@/lib/validation/schemas';
import { toast } from 'sonner';
import { ConfirmationDialog } from '@/components/ui/confirmation-dialog';
import ProblemSetCreationDialog from '@/components/problem-set-creation-dialog';
import AddToSetDialog from '@/components/add-to-set-dialog';
import {
  SearchFilters,
  Problem,
  SimpleTag,
  Tag,
  TagFilterMode,
} from '@/lib/types';
import { useIsMobile } from '@/lib/hooks/useMediaQuery';
import { useFilterParams } from '@/lib/hooks/useFilterParams';
import { confirmUnsavedNavigation } from '@/lib/hooks/useUnsavedChanges';
import ProblemCardList from '@/components/problems/problem-card-list';
import { validatePayload } from '@/lib/validation/payload';
import { clientApi } from '@/lib/api/client';

export default function EnhancedProblemsTable({
  initialProblems,
  initialTagsByProblem,
  subjectId,
  availableTags,
  onProblemDeleted = null,
  onEditProblem,
  onFiltersActiveChange,
  problemSetProblemIds = [],
  isAddToSetMode = false,
  targetProblemSetId = null,
}: {
  initialProblems: Problem[];
  initialTagsByProblem: Record<string, Tag[]>;
  subjectId: string;
  availableTags: SimpleTag[];
  onProblemDeleted?: ((problemId: string) => void) | null;
  onEditProblem?: (problem: Problem) => void;
  onFiltersActiveChange?: (active: boolean) => void;
  problemSetProblemIds?: string[];
  isAddToSetMode?: boolean;
  targetProblemSetId?: string | null;
}) {
  const router = useRouter();
  const isMobile = useIsMobile();
  const t = useTranslations('Problems');
  const tCommon = useTranslations('Common');
  const { initialFilters, updateUrl } = useFilterParams();
  const [deletedProblemIds, setDeletedProblemIds] = useState<Set<string>>(
    () => new Set()
  );
  const [error, setError] = useState<string | null>(null);

  // Search and filter state
  const [searchText, setSearchText] = useState(initialFilters.searchText);
  const [problemTypes, setProblemTypes] = useState<ProblemType[]>(
    initialFilters.problemTypes
  );
  const [tagIds, setTagIds] = useState<string[]>(initialFilters.tagIds);
  const [statuses, setStatuses] = useState<ProblemStatus[]>(
    initialFilters.statuses
  );
  const [tagFilterMode, setTagFilterMode] = useState<TagFilterMode>(
    initialFilters.tagFilterMode
  );

  // Bulk operations state
  const [selectedProblems, setSelectedProblems] = useState<Problem[]>([]);

  // Table instance state
  const [tableInstance, setTableInstance] = useState<any>(null);
  const [columnVisibilityKey, setColumnVisibilityKey] = useState(0);

  // Reset selection state
  const [resetSelection, setResetSelection] = useState(false);

  // Mobile select mode
  const [isSelectMode, setIsSelectMode] = useState(false);

  const problems = useMemo(
    () => initialProblems.filter(problem => !deletedProblemIds.has(problem.id)),
    [initialProblems, deletedProblemIds]
  );

  const tagsByProblem = useMemo(() => {
    if (deletedProblemIds.size === 0) return initialTagsByProblem;

    const nextTagsByProblem = { ...initialTagsByProblem };
    for (const problemId of deletedProblemIds) {
      delete nextTagsByProblem[problemId];
    }
    return nextTagsByProblem;
  }, [initialTagsByProblem, deletedProblemIds]);

  // Confirmation dialog state
  const [deleteDialog, setDeleteDialog] = useState<{
    open: boolean;
    problemId: string | null;
    problemTitle: string;
  }>({
    open: false,
    problemId: null,
    problemTitle: '',
  });

  // Bulk delete confirmation dialog state
  const [bulkDeleteDialog, setBulkDeleteDialog] = useState<{
    open: boolean;
    problemIds: string[];
    count: number;
  }>({
    open: false,
    problemIds: [],
    count: 0,
  });

  // Problem set creation dialog state
  const [createSetDialog, setCreateSetDialog] = useState<{
    open: boolean;
    problemIds: string[];
  }>({
    open: false,
    problemIds: [],
  });

  // Add to set dialog state
  const [addToSetDialog, setAddToSetDialog] = useState<{
    open: boolean;
    problem: Problem | null;
  }>({
    open: false,
    problem: null,
  });

  // Client-side filtering
  const filteredProblems = useMemo(() => {
    const q = searchText.trim().toLowerCase();
    const hasSearch = q.length > 0;
    const typeSet = problemTypes.length > 0 ? new Set(problemTypes) : null;
    const statusSet = statuses.length > 0 ? new Set(statuses) : null;
    const hasTagFilter = tagIds.length > 0;
    const tagIdSet = hasTagFilter ? new Set(tagIds) : null;

    return problems.filter(p => {
      if (hasSearch) {
        const pTags = tagsByProblem[p.id] || [];
        const matchTitle = p.title.toLowerCase().includes(q);
        const matchContent = p.content?.toLowerCase().includes(q);
        const matchSolution = p.solution_text?.toLowerCase().includes(q);
        const matchTags = pTags.some(tag => tag.name.toLowerCase().includes(q));
        if (!matchTitle && !matchContent && !matchSolution && !matchTags)
          return false;
      }

      if (typeSet && !typeSet.has(p.problem_type)) return false;

      if (hasTagFilter && tagIdSet) {
        const pTagIdSet = new Set((tagsByProblem[p.id] || []).map(t => t.id));
        if (tagFilterMode === 'all') {
          for (const id of tagIds) {
            if (!pTagIdSet.has(id)) return false;
          }
        } else {
          let hasAny = false;
          for (const id of tagIds) {
            if (pTagIdSet.has(id)) {
              hasAny = true;
              break;
            }
          }
          if (!hasAny) return false;
        }
      }

      if (statusSet && !statusSet.has(p.status)) return false;

      return true;
    });
  }, [
    problems,
    tagsByProblem,
    searchText,
    problemTypes,
    tagIds,
    tagFilterMode,
    statuses,
  ]);

  const tableProblems: Problem[] = useMemo(
    () =>
      filteredProblems.map(problem => ({
        id: problem.id,
        title: problem.title,
        content: problem.content || null,
        problem_type: problem.problem_type,
        correct_answer: problem.correct_answer || null,
        auto_mark: problem.auto_mark || false,
        status: problem.status,
        created_at: problem.created_at,
        updated_at: problem.updated_at,
        last_reviewed_date: problem.last_reviewed_date,
        subject_id: problem.subject_id,
        tags: tagsByProblem[problem.id] || [],
        isInSet: problemSetProblemIds.includes(problem.id),
      })),
    [filteredProblems, tagsByProblem, problemSetProblemIds]
  );

  const selectedProblemIds = useMemo(
    () => selectedProblems.map(problem => problem.id),
    [selectedProblems]
  );

  const columns = useMemo(() => createColumns(t), [t]);

  const hasActiveFilters =
    searchText.trim() !== '' ||
    problemTypes.length > 0 ||
    tagIds.length > 0 ||
    statuses.length > 0;

  useEffect(() => {
    onFiltersActiveChange?.(hasActiveFilters);
  }, [hasActiveFilters, onFiltersActiveChange]);

  const handleEdit = useCallback(
    (problem: Problem) => {
      if (onEditProblem) {
        onEditProblem(problem);
      }
    },
    [onEditProblem]
  );

  const handleSearch = useCallback(
    (filters: SearchFilters) => {
      updateUrl(filters);
    },
    [updateUrl]
  );

  const handleDeleteClick = (problemId: string, problemTitle: string) => {
    setDeleteDialog({
      open: true,
      problemId,
      problemTitle,
    });
  };

  const handleConfirmDelete = async () => {
    if (!deleteDialog.problemId) {
      toast.error(t('noProblemSelectedForDeletion'));
      return;
    }

    try {
      await clientApi(`/api/problems/${deleteDialog.problemId}`, {
        method: 'DELETE',
      });

      setDeletedProblemIds(prev => {
        const next = new Set(prev);
        next.add(deleteDialog.problemId!);
        return next;
      });

      if (onProblemDeleted) {
        onProblemDeleted(deleteDialog.problemId);
      }

      toast.success(t('problemDeletedSuccessfully'));
    } catch (err: any) {
      setError(err.message);
      toast.error(err.message);
    } finally {
      setDeleteDialog({ open: false, problemId: null, problemTitle: '' });
    }
  };

  const handleBulkDeleteClick = (problemIds: string[]) => {
    setBulkDeleteDialog({
      open: true,
      problemIds,
      count: problemIds.length,
    });
  };

  const handleCreateSetClick = async (problemIds: string[]) => {
    if (isAddToSetMode && targetProblemSetId) {
      try {
        await clientApi(`/api/problem-sets/${targetProblemSetId}/problems`, {
          method: 'POST',
          body: validatePayload(
            { problem_ids: problemIds },
            AddProblemsToSetDto,
            'add problems to set'
          ),
        });

        toast.success(t('addedProblemsToSet', { count: problemIds.length }));

        setSelectedProblems([]);
        setResetSelection(true);

        setTimeout(() => {
          router.push(`/problem-sets/${targetProblemSetId}`);
        }, 1000);
      } catch (error) {
        console.error('Error adding problems to set:', error);
        toast.error(
          error instanceof Error ? error.message : t('failedToAddProblemsToSet')
        );
      }
    } else {
      setCreateSetDialog({
        open: true,
        problemIds,
      });
    }
  };

  const handleAddToSetClick = (problem: Problem) => {
    setAddToSetDialog({
      open: true,
      problem,
    });
  };

  const handleConfirmBulkDelete = async () => {
    const { problemIds } = bulkDeleteDialog;

    try {
      const deletePromises = problemIds.map(problemId =>
        clientApi(`/api/problems/${problemId}`, {
          method: 'DELETE',
        }).then(() => {
          return problemId;
        })
      );

      await Promise.all(deletePromises);

      setDeletedProblemIds(prev => {
        const next = new Set(prev);
        for (const problemId of problemIds) {
          next.add(problemId);
        }
        return next;
      });

      problemIds.forEach(id => {
        if (onProblemDeleted) {
          onProblemDeleted(id);
        }
      });

      setSelectedProblems([]);
      setResetSelection(true);
      toast.success(
        t('problemsDeletedSuccessfully', { count: problemIds.length })
      );
    } catch (err: any) {
      setError(err.message);
      toast.error(t('failedToDeleteProblem', { error: err.message }));
    } finally {
      setBulkDeleteDialog({ open: false, problemIds: [], count: 0 });
    }
  };

  const handleSelectionChange = useCallback((selected: Problem[]) => {
    setSelectedProblems(selected);
  }, []);

  const getRowHref = useCallback(
    (problem: Problem) =>
      `/subjects/${problem.subject_id}/problems/${problem.id}/review`,
    []
  );

  const handleRowClick = useCallback(
    (problem: Problem) => {
      if (!confirmUnsavedNavigation()) return;
      router.push(getRowHref(problem));
    },
    [router, getRowHref]
  );

  useEffect(() => {
    if (resetSelection) {
      setResetSelection(false);
    }
  }, [resetSelection]);

  const handleColumnVisibilityChange = useCallback(() => {
    setColumnVisibilityKey(prev => prev + 1);
  }, []);

  return (
    <div className="space-y-4">
      <CompactSearchFilter
        onSearch={handleSearch}
        availableTags={availableTags}
        searchText={searchText}
        onSearchTextChange={setSearchText}
        problemTypes={problemTypes}
        onProblemTypesChange={setProblemTypes}
        tagIds={tagIds}
        onTagIdsChange={setTagIds}
        tagFilterMode={tagFilterMode}
        onTagFilterModeChange={setTagFilterMode}
        statuses={statuses}
        onStatusesChange={setStatuses}
        table={tableInstance}
        columnVisibilityKey={columnVisibilityKey}
        selectedProblemIds={selectedProblemIds}
        onBulkDelete={handleBulkDeleteClick}
        onBulkDeleteEnabled={!isAddToSetMode}
        onCreateSet={handleCreateSetClick}
        isAddToSetMode={isAddToSetMode}
        isSelectMode={isSelectMode}
        onSelectModeChange={setIsSelectMode}
      />

      {error && (
        <div className="rounded-lg border border-destructive bg-destructive/10 p-4">
          <p className="text-sm text-destructive">Error: {error}</p>
        </div>
      )}

      {isMobile ? (
        <ProblemCardList
          problems={tableProblems}
          isSelectMode={isSelectMode}
          selectedIds={selectedProblemIds}
          onSelectionChange={ids => {
            const selected = tableProblems.filter(p => ids.includes(p.id));
            setSelectedProblems(selected);
          }}
          onRowClick={handleRowClick}
          getRowHref={getRowHref}
          onEdit={onEditProblem ? handleEdit : undefined}
          onDelete={handleDeleteClick}
          onAddToSet={handleAddToSetClick}
          isAddToSetMode={isAddToSetMode}
        />
      ) : (
        <DataTable
          columns={columns}
          data={tableProblems}
          onEdit={onEditProblem ? handleEdit : undefined}
          onDelete={handleDeleteClick}
          onAddToSet={handleAddToSetClick}
          onRowClick={handleRowClick}
          getRowHref={getRowHref}
          availableTags={availableTags}
          onTableReady={setTableInstance}
          onSelectionChange={handleSelectionChange}
          resetSelection={resetSelection}
          onColumnVisibilityChange={handleColumnVisibilityChange}
          columnVisibilityStorageKey={`problems-table-column-visibility-${subjectId}`}
          isAddToSetMode={isAddToSetMode}
        />
      )}

      <ConfirmationDialog
        isOpen={deleteDialog.open}
        title={t('deleteProblemTitle')}
        message={t('deleteProblemMessage', {
          title: deleteDialog.problemTitle,
        })}
        confirmText={tCommon('delete')}
        cancelText={tCommon('cancel')}
        onConfirm={handleConfirmDelete}
        onCancel={() => setDeleteDialog(prev => ({ ...prev, open: false }))}
        variant="destructive"
      />

      <ConfirmationDialog
        isOpen={bulkDeleteDialog.open}
        title={t('deleteProblemsTitle')}
        message={t('deleteProblemsMessage', { count: bulkDeleteDialog.count })}
        confirmText={t('deleteAll')}
        cancelText={tCommon('cancel')}
        onConfirm={handleConfirmBulkDelete}
        onCancel={() => setBulkDeleteDialog(prev => ({ ...prev, open: false }))}
        variant="destructive"
      />

      <ProblemSetCreationDialog
        open={createSetDialog.open}
        onOpenChange={open => setCreateSetDialog(prev => ({ ...prev, open }))}
        subjectId={subjectId}
        selectedProblemIds={createSetDialog.problemIds}
        onSuccess={() => {
          setSelectedProblems([]);
          setResetSelection(true);
        }}
      />

      {addToSetDialog.problem && (
        <AddToSetDialog
          open={addToSetDialog.open}
          onOpenChange={open => setAddToSetDialog(prev => ({ ...prev, open }))}
          problemId={addToSetDialog.problem.id}
          subjectId={subjectId}
          onSuccess={() => {}}
        />
      )}
    </div>
  );
}

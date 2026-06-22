'use client';

import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { useTranslations } from 'next-intl';
import { CreateTagDto } from '@/lib/validation/schemas';
import { validatePayload } from '@/lib/validation/payload';
import { clientApi, ClientApiError } from '@/lib/api/client';
import type { Problem, SimpleTag, SuggestedTags, Tag } from '@/lib/types';

interface UseTagPickerOptions {
  subjectId: string;
  availableTags: SimpleTag[];
  problem: Problem | null;
}

export function useTagPicker({
  subjectId,
  availableTags,
  problem,
}: UseTagPickerOptions) {
  const t = useTranslations('Subjects');

  const transformSimpleTagsToTags = useCallback(
    (simpleTags: SimpleTag[]): Tag[] =>
      simpleTags?.map(tag => ({
        ...tag,
        subject_id: subjectId,
        created_at: new Date().toISOString(),
      })) || [],
    [subjectId]
  );

  const [tags, setTags] = useState<Tag[]>(
    transformSimpleTagsToTags(availableTags)
  );
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>(() => {
    if (problem && problem.tags) {
      return problem.tags.map((tag: any) => tag.id);
    }
    return [];
  });
  const [pendingNewTags, setPendingNewTags] = useState<string[]>([]);
  const [deselectedPendingTags, setDeselectedPendingTags] = useState<
    Set<string>
  >(new Set());
  const [newTagName, setNewTagName] = useState('');
  const [creatingTag, setCreatingTag] = useState(false);

  useEffect(() => {
    if (availableTags && availableTags.length > 0) {
      setTags(transformSimpleTagsToTags(availableTags));
    } else {
      clientApi<Tag[]>(`/api/tags?subject_id=${subjectId}`)
        .then(setTags)
        .catch(() => {});
    }
  }, [availableTags, subjectId, transformSimpleTagsToTags]);

  function toggleTag(id: string) {
    setSelectedTagIds(prev =>
      prev.includes(id) ? prev.filter(x => x !== id) : [...prev, id]
    );
  }

  async function handleCreateTag() {
    const trimmed = newTagName.trim();
    if (!trimmed) return;
    if (tags.some(tag => tag.name.toLowerCase() === trimmed.toLowerCase())) {
      toast.error(t('tagExists'));
      return;
    }

    setCreatingTag(true);
    try {
      const created = await clientApi<Tag>('/api/tags', {
        method: 'POST',
        body: validatePayload(
          { subject_id: subjectId, name: trimmed },
          CreateTagDto,
          'create tag'
        ),
      });

      setTags(prev => [...prev, created]);
      setSelectedTagIds(prev => [...prev, created.id]);
      setNewTagName('');
      toast.success(t('tagCreated'));
    } catch (e: any) {
      toast.error(e.message || t('couldNotCreateTag'));
    } finally {
      setCreatingTag(false);
    }
  }

  function applySuggestedTags(suggestedTags: SuggestedTags | undefined) {
    if (!suggestedTags) return;

    const existingIds = suggestedTags.existing.map(tag => tag.id);
    setSelectedTagIds(prev => Array.from(new Set([...prev, ...existingIds])));

    const newNames = suggestedTags.new
      .map(tag => tag.name)
      .filter(
        name => !tags.some(tag => tag.name.toLowerCase() === name.toLowerCase())
      );
    setPendingNewTags(newNames);
    setDeselectedPendingTags(new Set());
  }

  async function createPendingTagsForSubmit(): Promise<string[]> {
    const finalTagIds = [...selectedTagIds];
    const activePendingTags = pendingNewTags.filter(
      name => !deselectedPendingTags.has(name)
    );
    const createdTagNames: string[] = [];

    for (const tagName of activePendingTags) {
      const existing = tags.find(
        tag => tag.name.toLowerCase() === tagName.toLowerCase()
      );
      if (existing) {
        if (!finalTagIds.includes(existing.id)) finalTagIds.push(existing.id);
        createdTagNames.push(tagName);
        continue;
      }

      try {
        const created = await clientApi<Tag>('/api/tags', {
          method: 'POST',
          body: validatePayload(
            { subject_id: subjectId, name: tagName },
            CreateTagDto,
            'create tag'
          ),
        });
        setTags(prev => [...prev, created]);
        finalTagIds.push(created.id);
        createdTagNames.push(tagName);
      } catch (error) {
        if (error instanceof ClientApiError && error.status === 403) {
          toast.warning(
            `${t('couldNotCreateTag')} "${tagName}": ${t('tagLimitReached')}`
          );
        } else {
          toast.warning(`${t('couldNotCreateTag')} "${tagName}"`);
        }
      }
    }

    if (activePendingTags.length > 0) {
      setSelectedTagIds(finalTagIds);
      setPendingNewTags(prev =>
        prev.filter(name => !createdTagNames.includes(name))
      );
      setDeselectedPendingTags(prev => {
        const next = new Set(prev);
        for (const name of createdTagNames) next.delete(name);
        return next;
      });
    }

    return finalTagIds;
  }

  function resetAfterCreate() {
    setSelectedTagIds([]);
    setPendingNewTags([]);
    setDeselectedPendingTags(new Set());
  }

  return {
    tags,
    setTags,
    selectedTagIds,
    setSelectedTagIds,
    pendingNewTags,
    setPendingNewTags,
    deselectedPendingTags,
    setDeselectedPendingTags,
    newTagName,
    setNewTagName,
    creatingTag,
    toggleTag,
    handleCreateTag,
    applySuggestedTags,
    createPendingTagsForSubmit,
    resetAfterCreate,
  };
}

export type TagPickerState = ReturnType<typeof useTagPicker>;

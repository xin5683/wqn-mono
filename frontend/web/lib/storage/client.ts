import { FILE_CONSTANTS } from '../constants';
import {
  getApiErrorMessage,
  readApiResponseBody,
  unwrapApiData,
} from '../api/errors';

export async function uploadFiles(
  files: FileList | File[],
  role: 'problem' | 'solution',
  problemId: string
): Promise<string[]> {
  if (!problemId || problemId.trim() === '') {
    throw new Error('Problem ID is required for file upload');
  }
  if (problemId.includes('/') || problemId.includes('\\')) {
    throw new Error('Invalid Problem ID: contains path separators');
  }

  const maxSize = FILE_CONSTANTS.MAX_FILE_SIZE.GENERAL;
  const oversizedFiles: string[] = [];
  Array.from(files).forEach(file => {
    if (file.size > maxSize) {
      oversizedFiles.push(file.name);
    }
  });
  if (oversizedFiles.length > 0) {
    throw new Error(
      `Files too large: ${oversizedFiles.join(', ')}. Maximum file size is 10MB.`
    );
  }

  const formData = new FormData();
  formData.append('role', role);
  formData.append('problem_id', problemId);
  for (const file of Array.from(files)) {
    formData.append('files[]', file, file.name.replace(/\s+/g, '_'));
  }

  const response = await fetch('/api/files/upload', {
    method: 'POST',
    credentials: 'include',
    body: formData,
  });
  const body = await readApiResponseBody(response);
  if (!response.ok) {
    throw new Error(getApiErrorMessage(body, 'Upload failed'));
  }
  const data = unwrapApiData(body) as { paths?: string[] } | null;
  return data?.paths || [];
}

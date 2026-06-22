# Local Storage Operations

The Rust backend stores files under `LOCAL_STORAGE_ROOT`:

- `problem-uploads/` for problem and solution assets.
- `avatars/` for profile avatars.

## Upload Safety

The API enforces:

- Problem assets: JPEG, PNG, WebP, GIF, or PDF only.
- Avatars: JPEG, PNG, WebP, or GIF only.
- Problem asset size limit: 10 MB.
- Avatar size limit: 2 MB.

Set `LOCAL_STORAGE_SCAN_COMMAND` to enable a content safety scanner. The backend
writes each upload to a temporary file, runs the command with the temporary path
appended as the final argument, and only commits the file if the command exits
successfully.

Example:

```bash
LOCAL_STORAGE_SCAN_COMMAND="clamscan --no-summary"
```

If no scanner is configured, the MIME allowlist and size limits still apply.

## Backups

Run:

```bash
LOCAL_STORAGE_ROOT=/var/lib/wqn/storage \
STORAGE_BACKUP_DIR=/var/backups/wqn-storage \
scripts/storage-backup.sh
```

The script creates a timestamped `tar.gz` archive and a SHA-256 manifest.
`STORAGE_BACKUP_RETENTION_DAYS` defaults to `14`; set it to `0` to disable
automatic old-backup deletion.

Restore by stopping the API, extracting the archive into an empty storage root,
and starting the API again.

## Orphan Cleanup

Run a dry run first:

```bash
DATABASE_URL=postgres://wqn:wqn@127.0.0.1:55432/wqn \
LOCAL_STORAGE_ROOT=/var/lib/wqn/storage \
scripts/storage-cleanup-orphans.sh
```

The cleanup script compares files on disk with paths referenced from
`problems.assets`, `problems.solution_assets`, `qr_upload_sessions.file_path`,
and `user_profiles.avatar_url`.

Delete after reviewing the dry-run output:

```bash
APPLY=1 scripts/storage-cleanup-orphans.sh
```

Use the cleanup script after database restores, failed uploads, or manual file
maintenance. Keep at least one recent backup before running with `APPLY=1`.

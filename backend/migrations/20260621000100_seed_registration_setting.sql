-- Seed the operational `user_registration` admin setting so the toggle is
-- visible in the admin settings UI and self-service sign-up is open by
-- default. A super-admin can flip `enabled` to false via
-- PATCH /admin/settings/user_registration to close registration; the
-- `sign_up` handler reads it and rejects new sign-ups while closed.
insert into admin_settings (key, value, description)
values (
  'user_registration',
  '{"enabled": true, "require_email_verification": false}'::jsonb,
  'Self-service user registration'
)
on conflict (key) do nothing;

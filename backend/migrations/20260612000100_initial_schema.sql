create extension if not exists pgcrypto;

create or replace function set_updated_at()
returns trigger language plpgsql as $$
begin
  new.updated_at = now();
  return new;
end;
$$;

create table if not exists app_users (
  id uuid primary key default gen_random_uuid(),
  email text not null,
  password_hash text not null,
  token_version integer not null default 0,
  disabled_at timestamptz,
  last_login_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create unique index if not exists app_users_lower_email_idx on app_users (lower(email));

create table if not exists user_profiles (
  id uuid primary key,
  username text unique,
  first_name text,
  last_name text,
  date_of_birth date,
  gender text,
  region text,
  timezone text not null default 'UTC',
  avatar_url text,
  bio text,
  user_role text not null default 'user' check (user_role in ('user', 'moderator', 'admin', 'super_admin')),
  is_active boolean not null default true,
  last_login_at timestamptz,
  onboarding_completed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists subjects (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  name text not null,
  color text,
  icon text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, name)
);

create table if not exists tags (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  subject_id uuid not null references subjects(id) on delete cascade,
  name text not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, subject_id, name)
);

create table if not exists problems (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  subject_id uuid not null references subjects(id) on delete cascade,
  title text not null,
  content text,
  problem_type text not null check (problem_type in ('mcq', 'short', 'extended')),
  correct_answer text,
  answer_config jsonb,
  auto_mark boolean not null default false,
  status text not null default 'needs_review' check (status in ('wrong', 'needs_review', 'mastered')),
  assets jsonb not null default '[]'::jsonb,
  solution_text text,
  solution_assets jsonb not null default '[]'::jsonb,
  last_reviewed_date timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists problem_tag (
  user_id uuid not null,
  problem_id uuid not null references problems(id) on delete cascade,
  tag_id uuid not null references tags(id) on delete cascade,
  created_at timestamptz not null default now(),
  primary key (problem_id, tag_id)
);

create table if not exists attempts (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  problem_id uuid not null references problems(id) on delete cascade,
  submitted_answer jsonb not null,
  is_correct boolean,
  cause text,
  is_self_assessed boolean not null default false,
  confidence integer check (confidence is null or confidence between 1 and 5),
  reflection_notes text,
  selected_status text check (selected_status is null or selected_status in ('wrong', 'needs_review', 'mastered')),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists review_schedule (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  problem_id uuid not null references problems(id) on delete cascade,
  next_review_at timestamptz not null default now(),
  interval_days integer not null default 1,
  ease_factor double precision not null default 2.5,
  repetition_number integer not null default 0,
  last_reviewed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, problem_id)
);

create table if not exists problem_sets (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  subject_id uuid not null references subjects(id) on delete cascade,
  name text not null,
  description text,
  sharing_level text not null default 'private' check (sharing_level in ('private', 'limited', 'public')),
  is_smart boolean not null default false,
  filter_config jsonb,
  session_config jsonb,
  allow_copying boolean not null default true,
  is_listed boolean not null default false,
  discovery_subject text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists problem_set_problems (
  problem_set_id uuid not null references problem_sets(id) on delete cascade,
  problem_id uuid not null references problems(id) on delete cascade,
  user_id uuid not null,
  added_at timestamptz not null default now(),
  primary key (problem_set_id, problem_id)
);

create table if not exists problem_set_shares (
  id uuid primary key default gen_random_uuid(),
  problem_set_id uuid not null references problem_sets(id) on delete cascade,
  shared_with_email text not null,
  shared_by_user_id uuid not null,
  created_at timestamptz not null default now()
);

create unique index if not exists problem_set_shares_email_idx
  on problem_set_shares (problem_set_id, lower(shared_with_email));

create table if not exists problem_set_likes (
  id uuid primary key default gen_random_uuid(),
  problem_set_id uuid not null references problem_sets(id) on delete cascade,
  user_id uuid not null,
  created_at timestamptz not null default now(),
  unique (problem_set_id, user_id)
);

create table if not exists problem_set_favourites (
  id uuid primary key default gen_random_uuid(),
  problem_set_id uuid not null references problem_sets(id) on delete cascade,
  user_id uuid not null,
  created_at timestamptz not null default now(),
  unique (problem_set_id, user_id)
);

create table if not exists problem_set_reports (
  id uuid primary key default gen_random_uuid(),
  problem_set_id uuid not null references problem_sets(id) on delete cascade,
  reported_by_user_id uuid not null,
  reason text not null,
  details text,
  status text not null default 'open',
  created_at timestamptz not null default now()
);

create table if not exists problem_set_views (
  id uuid primary key default gen_random_uuid(),
  problem_set_id uuid not null references problem_sets(id) on delete cascade,
  viewer_user_id uuid,
  ip_hash text,
  created_at timestamptz not null default now()
);

create unique index if not exists problem_set_views_dedupe_idx
  on problem_set_views (
    problem_set_id,
    (coalesce(viewer_user_id, '00000000-0000-0000-0000-000000000000'::uuid)),
    (coalesce(ip_hash, ''))
  );

create table if not exists problem_set_copies (
  id uuid primary key default gen_random_uuid(),
  source_problem_set_id uuid references problem_sets(id) on delete set null,
  source_problem_id uuid references problems(id) on delete set null,
  copied_by_user_id uuid not null,
  new_problem_set_id uuid references problem_sets(id) on delete set null,
  new_problem_id uuid references problems(id) on delete set null,
  created_at timestamptz not null default now()
);

create table if not exists review_session_state (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  session_type text not null,
  subject_id uuid references subjects(id) on delete set null,
  problem_set_id uuid references problem_sets(id) on delete set null,
  session_state jsonb not null default '{}'::jsonb,
  is_active boolean not null default true,
  last_activity_at timestamptz not null default now(),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists review_session_results (
  id uuid primary key default gen_random_uuid(),
  session_state_id uuid not null references review_session_state(id) on delete cascade,
  problem_id uuid not null references problems(id) on delete cascade,
  was_correct boolean,
  was_skipped boolean not null default false,
  completed_at timestamptz not null default now()
);

create table if not exists usage_quotas (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  resource_type text not null,
  period_start date not null,
  usage_count bigint not null default 0,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, resource_type, period_start)
);

create table if not exists user_quota_overrides (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  resource_type text not null,
  daily_limit bigint not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, resource_type)
);

create table if not exists content_limit_overrides (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  resource_type text not null,
  limit_value bigint not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, resource_type)
);

create table if not exists qr_upload_sessions (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  token_hash text not null,
  status text not null default 'pending' check (status in ('pending', 'uploaded', 'consumed', 'expired')),
  file_path text,
  mime_type text,
  expires_at timestamptz not null,
  uploaded_at timestamptz,
  consumed_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists admin_settings (
  id uuid primary key default gen_random_uuid(),
  key text not null unique,
  value jsonb not null default '{}'::jsonb,
  description text,
  updated_by uuid,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists user_activity_log (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  action text not null,
  resource_type text,
  resource_id uuid,
  details jsonb,
  ip_address text,
  user_agent text,
  created_at timestamptz not null default now()
);

create table if not exists error_categorisations (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  attempt_id uuid not null references attempts(id) on delete cascade,
  problem_id uuid not null references problems(id) on delete cascade,
  subject_id uuid not null references subjects(id) on delete cascade,
  broad_category text,
  granular_tag text,
  topic_label text,
  confidence double precision,
  reasoning text,
  model_response jsonb,
  is_user_override boolean not null default false,
  original_broad_category text,
  original_granular_tag text,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (attempt_id)
);

create table if not exists insight_digests (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null,
  status text not null default 'generating' check (status in ('generating', 'completed', 'failed')),
  digest jsonb,
  error_message text,
  generated_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create index if not exists subjects_user_idx on subjects(user_id);
create index if not exists tags_user_subject_idx on tags(user_id, subject_id);
create index if not exists problems_user_subject_idx on problems(user_id, subject_id);
create index if not exists attempts_user_problem_idx on attempts(user_id, problem_id);
create index if not exists problem_sets_user_subject_idx on problem_sets(user_id, subject_id);
create index if not exists review_schedule_due_idx on review_schedule(user_id, next_review_at);
create index if not exists review_session_user_active_idx on review_session_state(user_id, is_active);
create index if not exists insight_digests_user_created_idx on insight_digests(user_id, created_at desc);

drop view if exists problem_set_stats;
create view problem_set_stats as
select
  ps.id as problem_set_id,
  (select count(*)::bigint from problem_set_views where problem_set_id = ps.id) as view_count,
  (select count(*)::bigint from problem_set_likes where problem_set_id = ps.id) as like_count,
  (select count(*)::bigint from problem_set_favourites where problem_set_id = ps.id) as favourite_count,
  (select count(*)::bigint from problem_set_copies where source_problem_set_id = ps.id) as copy_count
from problem_sets ps;

drop view if exists discoverable_problem_sets;
create view discoverable_problem_sets as
select
  ps.*,
  s.name as subject_name,
  up.username as owner_username,
  up.avatar_url as owner_avatar_url,
  stats.view_count,
  stats.like_count,
  stats.favourite_count,
  stats.copy_count
from problem_sets ps
left join subjects s on s.id = ps.subject_id
left join user_profiles up on up.id = ps.user_id
left join problem_set_stats stats on stats.problem_set_id = ps.id
where ps.sharing_level = 'public' and coalesce(ps.is_listed, true) = true;

create or replace function get_subjects_with_metadata()
returns setof jsonb language sql stable as $$
  select to_jsonb(s)
    || jsonb_build_object(
      'problem_count', count(distinct p.id),
      'tag_count', count(distinct t.id)
    )
  from subjects s
  left join problems p on p.subject_id = s.id and p.user_id = s.user_id
  left join tags t on t.subject_id = s.id and t.user_id = s.user_id
  group by s.id
  order by s.name asc;
$$;

create or replace function get_user_storage_bytes(p_user_id uuid)
returns bigint language sql stable as $$
  with files as (
    select jsonb_array_elements(coalesce(assets, '[]'::jsonb)) asset
    from problems where user_id = p_user_id
    union all
    select jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) asset
    from problems where user_id = p_user_id
  )
  select coalesce(sum(nullif(asset->>'size', '')::bigint), 0) from files;
$$;

create or replace function find_problem_by_asset(p_path text)
returns uuid language sql stable as $$
  select id from problems
  where exists (
    select 1 from jsonb_array_elements(coalesce(assets, '[]'::jsonb)) item
    where item->>'path' = p_path
  )
  or exists (
    select 1 from jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) item
    where item->>'path' = p_path
  )
  limit 1;
$$;

create or replace function can_view_problem(p_problem_id uuid)
returns boolean language sql stable as $$
  select exists(
    select 1
    from problem_set_problems psp
    join problem_sets ps on ps.id = psp.problem_set_id
    where psp.problem_id = p_problem_id and ps.sharing_level = 'public'
  );
$$;

create or replace function user_owns_problem_with_asset(p_path text)
returns boolean language sql stable as $$
  select exists(
    select 1 from problems
    where exists (
      select 1 from jsonb_array_elements(coalesce(assets, '[]'::jsonb)) item
      where item->>'path' = p_path
    )
    or exists (
      select 1 from jsonb_array_elements(coalesce(solution_assets, '[]'::jsonb)) item
      where item->>'path' = p_path
    )
  );
$$;

create or replace function record_problem_set_view(
  p_problem_set_id uuid,
  p_viewer_user_id uuid default null,
  p_ip_hash text default null
)
returns void language sql as $$
  insert into problem_set_views (problem_set_id, viewer_user_id, ip_hash)
  values (p_problem_set_id, p_viewer_user_id, p_ip_hash)
  on conflict (
    problem_set_id,
    (coalesce(viewer_user_id, '00000000-0000-0000-0000-000000000000'::uuid)),
    (coalesce(ip_hash, ''))
  ) do nothing;
$$;

create or replace function toggle_problem_set_like(
  p_problem_set_id uuid,
  p_user_id uuid
)
returns jsonb language plpgsql as $$
declare
  deleted_count integer;
  new_count bigint;
begin
  delete from problem_set_likes
  where problem_set_id = p_problem_set_id and user_id = p_user_id;
  get diagnostics deleted_count = row_count;

  if deleted_count = 0 then
    insert into problem_set_likes (problem_set_id, user_id)
    values (p_problem_set_id, p_user_id)
    on conflict do nothing;
  end if;

  select count(*) into new_count
  from problem_set_likes where problem_set_id = p_problem_set_id;

  return jsonb_build_object('liked', deleted_count = 0, 'like_count', new_count);
end;
$$;

create or replace function record_problem_set_copy(
  p_source_problem_set_id uuid,
  p_copied_by_user_id uuid,
  p_new_problem_set_id uuid default null
)
returns void language sql as $$
  insert into problem_set_copies (source_problem_set_id, copied_by_user_id, new_problem_set_id)
  values (p_source_problem_set_id, p_copied_by_user_id, p_new_problem_set_id);
$$;

create or replace function log_user_activity(
  p_user_id uuid,
  p_action text,
  p_resource_type text default null,
  p_resource_id uuid default null,
  p_details jsonb default null
)
returns void language sql as $$
  insert into user_activity_log (user_id, action, resource_type, resource_id, details)
  values (p_user_id, p_action, p_resource_type, p_resource_id, p_details);
$$;

drop trigger if exists set_user_profiles_updated_at on user_profiles;
create trigger set_user_profiles_updated_at before update on user_profiles
for each row execute function set_updated_at();
drop trigger if exists set_app_users_updated_at on app_users;
create trigger set_app_users_updated_at before update on app_users
for each row execute function set_updated_at();
drop trigger if exists set_subjects_updated_at on subjects;
create trigger set_subjects_updated_at before update on subjects
for each row execute function set_updated_at();
drop trigger if exists set_tags_updated_at on tags;
create trigger set_tags_updated_at before update on tags
for each row execute function set_updated_at();
drop trigger if exists set_problems_updated_at on problems;
create trigger set_problems_updated_at before update on problems
for each row execute function set_updated_at();
drop trigger if exists set_attempts_updated_at on attempts;
create trigger set_attempts_updated_at before update on attempts
for each row execute function set_updated_at();
drop trigger if exists set_problem_sets_updated_at on problem_sets;
create trigger set_problem_sets_updated_at before update on problem_sets
for each row execute function set_updated_at();
drop trigger if exists set_review_session_state_updated_at on review_session_state;
create trigger set_review_session_state_updated_at before update on review_session_state
for each row execute function set_updated_at();
drop trigger if exists set_usage_quotas_updated_at on usage_quotas;
create trigger set_usage_quotas_updated_at before update on usage_quotas
for each row execute function set_updated_at();
drop trigger if exists set_user_quota_overrides_updated_at on user_quota_overrides;
create trigger set_user_quota_overrides_updated_at before update on user_quota_overrides
for each row execute function set_updated_at();
drop trigger if exists set_content_limit_overrides_updated_at on content_limit_overrides;
create trigger set_content_limit_overrides_updated_at before update on content_limit_overrides
for each row execute function set_updated_at();
drop trigger if exists set_qr_upload_sessions_updated_at on qr_upload_sessions;
create trigger set_qr_upload_sessions_updated_at before update on qr_upload_sessions
for each row execute function set_updated_at();
drop trigger if exists set_admin_settings_updated_at on admin_settings;
create trigger set_admin_settings_updated_at before update on admin_settings
for each row execute function set_updated_at();
drop trigger if exists set_error_categorisations_updated_at on error_categorisations;
create trigger set_error_categorisations_updated_at before update on error_categorisations
for each row execute function set_updated_at();
drop trigger if exists set_insight_digests_updated_at on insight_digests;
create trigger set_insight_digests_updated_at before update on insight_digests
for each row execute function set_updated_at();

CREATE TABLE users (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),

  email TEXT NOT NULL,
  display_name TEXT NOT NULL,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE UNIQUE INDEX idx_unique_users_on_email
  ON users(email);

CREATE TABLE oauth_provider_accounts (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),

  user_id BLOB NOT NULL
    REFERENCES users(id)
    ON DELETE CASCADE,

  provider TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  provider_email TEXT NOT NULL,

  associated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE UNIQUE INDEX idx_unique_oauth_provider_accounts_on_provider_provider_id
  ON oauth_provider_accounts(provider, provider_id);
CREATE UNIQUE INDEX idx_unique_oauth_provider_accounts_on_provider_provider_email
  ON oauth_provider_accounts(provider, provider_email);

CREATE TABLE oauth_state (
  provider TEXT NOT NULL,

  csrf_token_secret TEXT NOT NULL,
  pkce_code_verifier_secret TEXT NOT NULL,

  post_login_redirect_url TEXT,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE UNIQUE INDEX idx_unique_oauth_state_on_provider_csrf_token_secret
  ON oauth_state(provider, csrf_token_secret);

CREATE TABLE sessions (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),

  user_id BLOB NOT NULL
    REFERENCES users(id)
    ON DELETE CASCADE,

  oauth_provider_account_id BLOB NOT NULL
    REFERENCES oauth_provider_accounts(id)
    ON DELETE CASCADE,

  client_ip TEXT,
  user_agent TEXT,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  expires_at TEXT NOT NULL
) STRICT;

CREATE TABLE api_keys (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),

  user_id BLOB NOT NULL
    REFERENCES users(id)
    ON DELETE CASCADE,

  name TEXT,
  fingerprint BLOB NOT NULL,
  public_key BLOB NOT NULL,

  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE INDEX idx_api_keys_on_user_id
  ON api_keys(user_id);
CREATE UNIQUE INDEX idx_unqiue_api_keys_on_fingerprint
  ON api_keys(fingerprint);

CREATE TABLE background_jobs (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),

  name TEXT NOT NULL,
  queue_name TEXT NOT NULL DEFAULT 'default',

  unique_key BLOB,
  state TEXT NOT NULL,

  current_attempt INTEGER NOT NULL DEFAULT 1,
  maximum_attempts INTEGER NOT NULL,

  payload BLOB NOT NULL,

  job_scheduled_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  attempt_run_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE INDEX idx_background_jobs_on_attempt_run_at ON background_jobs(attempt_run_at);
CREATE INDEX idx_background_jobs_on_scheduled_at ON background_jobs(job_scheduled_at);
CREATE INDEX idx_background_jobs_on_state ON background_jobs(state);
CREATE INDEX idx_background_jobs_on_name ON background_jobs(name);
CREATE INDEX idx_background_jobs_on_queue_name ON background_jobs(queue_name);

-- Uniqueness is only required on active jobs, specifically this is requiring
-- the uniqueness on tasks that are new, started, or retrying.
CREATE UNIQUE INDEX idx_background_jobs_on_name_unique_key
  ON background_jobs(name, unique_key)
  WHERE unique_key != NULL AND state IN (1, 2, 3);

CREATE TABLE job_run (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (randomblob(16)),

  background_job_id BLOB NOT NULL
    REFERENCES background_jobs(id)
    ON DELETE CASCADE,

  result TEXT NOT NULL,
  output BLOB,

  run_started_at TEXT,
  run_finished_at TEXT
) STRICT;

CREATE INDEX idx_job_run_on_background_job_id ON job_run(background_job_id);

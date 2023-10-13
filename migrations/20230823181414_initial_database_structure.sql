CREATE TABLE users (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (
    randomblob(6) ||
    (randomblob(1) | X'40') ||
    (randomblob(1) & X'0F' | X'80') ||
    randomblob(4)
  ),

  email TEXT NOT NULL,
  display_name TEXT NOT NULL,

  -- todo: probably want to normalize locale and check it
  locale TEXT,
  profile_image TEXT,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_unique_users_on_email ON
  users(email);

CREATE TABLE oauth_state (
  provider TEXT NOT NULL,

  csrf_token_secret TEXT NOT NULL,
  pkce_code_verifier_secret TEXT NOT NULL,

  post_login_redirect_url TEXT,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_unique_oauth_state_on_provider_csrf_token_secret
  ON oauth_state(provider, csrf_token_secret);

CREATE TABLE sessions (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (
    randomblob(6) ||
    (randomblob(1) | X'40') ||
    (randomblob(1) & X'0F' | X'80') ||
    randomblob(4)
  ),

  user_id BLOB NOT NULL
    REFERENCES users(id)
    ON DELETE CASCADE,

  provider TEXT NOT NULL,
  access_token_secret TEXT NOT NULL,
  access_expires_at TIMESTAMP,
  refresh_token TEXT,

  client_ip BLOB,
  user_agent TEXT,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  expires_at TIMESTAMP NOT NULL
);

CREATE TABLE api_keys (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (
    randomblob(6) ||
    (randomblob(1) | X'40') ||
    (randomblob(1) & X'0F' | X'80') ||
    randomblob(4)
  ),

  user_id BLOB NOT NULL
    REFERENCES users(id)
    ON DELETE CASCADE,

  fingerprint BLOB NOT NULL,
  public_key BLOB NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE background_jobs (
  id BLOB NOT NULL PRIMARY KEY DEFAULT (
    randomblob(6) ||
    (randomblob(1) | X'40') ||
    (randomblob(1) & X'0F' | X'80') ||
    randomblob(4)
  ),

  name TEXT NOT NULL,
  queue_name TEXT NOT NULL DEFAULT 'default',

  unique_key BLOB,
  state INTEGER NOT NULL,

  current_attempt INTEGER NOT NULL DEFAULT 1,
  maximum_attempts INTEGER NOT NULL,

  payload BLOB NOT NULL,

  job_scheduled_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  attempt_run_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

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
  id BLOB NOT NULL PRIMARY KEY DEFAULT (
    randomblob(6) ||
    (randomblob(1) | X'40') ||
    (randomblob(1) & X'0F' | X'80') ||
    randomblob(4)
  ),

  background_job_id BLOB NOT NULL
    REFERENCES background_jobs(id)
    ON DELETE CASCADE,

  result NUMERIC NOT NULL,
  output BLOB,

  run_started_at TIMESTAMP,
  run_finished_at TIMESTAMP
);

CREATE INDEX idx_job_run_on_background_job_id ON job_run(background_job_id);

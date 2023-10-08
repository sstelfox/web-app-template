CREATE TABLE users (
  id TEXT NOT NULL PRIMARY KEY DEFAULT (
    lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-a' ||
    substr(lower(hex(randomblob(2))), 2) || '-6' ||
    substr(lower(hex(randomblob(6))), 2)
  ),

  email TEXT NOT NULL,
  display_name TEXT NOT NULL,
  locale TEXT,
  profile_image TEXT,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_unique_users_on_email ON
  users(email);

CREATE TABLE oauth_state (
  provider TEXT NOT NULL,
  csrf_secret TEXT NOT NULL,
  pkce_verifier_secret TEXT NOT NULL,

  post_login_redirect_url TEXT,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_unique_oauth_state_on_provider_csrf_secret
  ON oauth_state(provider, csrf_secret);

CREATE TABLE sessions (
  id TEXT NOT NULL PRIMARY KEY DEFAULT (
    lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-a' ||
    substr(lower(hex(randomblob(2))), 2) || '-6' ||
    substr(lower(hex(randomblob(6))), 2)
  ),

  user_id TEXT NOT NULL
    REFERENCES users(id)
    ON DELETE CASCADE,

  provider TEXT NOT NULL,
  access_token TEXT NOT NULL,
  access_expires_at TIMESTAMP,
  refresh_token TEXT,

  client_ip TEXT,
  user_agent TEXT,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  expires_at TIMESTAMP NOT NULL
);

CREATE TABLE api_keys (
  id TEXT NOT NULL PRIMARY KEY DEFAULT (
    lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-a' ||
    substr(lower(hex(randomblob(2))), 2) || '-6' ||
    substr(lower(hex(randomblob(6))), 2)
  ),

  user_id TEXT NOT NULL
    REFERENCES users(id)
    ON DELETE CASCADE,

  fingerprint TEXT NOT NULL,
  public_key BLOB NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE background_tasks (
  id TEXT NOT NULL PRIMARY KEY DEFAULT (
    lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-a' ||
    substr(lower(hex(randomblob(2))), 2) || '-6' ||
    substr(lower(hex(randomblob(6))), 2)
  ),

  task_queue TEXT NOT NULL DEFAULT 'default',
  state TEXT CHECK (state IN ('new', 'in_progress', 'cancelled', 'failed', 'complete', 'dead')) NOT NULL DEFAULT 'new',
  retry_count INTEGER NOT NULL DEFAULT 0,
  uniq_hash TEXT,

  -- should probably be structured, for now will be JSON blob
  error TEXT,

  -- actually a JSON blob might want to split more of this out
  metadata TEXT NOT NULL,

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  scheduled_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,

  started_at TIMESTAMP,
  ended_at TIMESTAMP
);

CREATE INDEX background_tasks_on_scheduled_at_idx ON background_tasks(scheduled_at);
CREATE INDEX background_tasks_on_state_idx ON background_tasks(state);
CREATE INDEX background_tasks_on_task_queue_idx ON background_tasks(task_queue);
CREATE INDEX background_tasks_on_uniq_hash_idx ON background_tasks(uniq_hash) WHERE uniq_hash != NULL;

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
  uniq_hash CHAR(64),

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

CREATE TABLE users (
  id TEXT NOT NULL PRIMARY KEY DEFAULT (
    lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-a' ||
    substr(lower(hex(randomblob(2))), 2) || '-6' ||
    substr(lower(hex(randomblob(6))), 2)
  ),

  email VARCHAR(128) NOT NULL,
  display_name VARCHAR(128) NOT NULL,

  picture VARCHAR(256),
  locale VARCHAR(16),

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE oauth_state (
  id TEXT NOT NULL PRIMARY KEY DEFAULT (
    lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-a' ||
    substr(lower(hex(randomblob(2))), 2) || '-6' ||
    substr(lower(hex(randomblob(6))), 2)
  ),

  csrf_secret TEXT NOT NULL,
  pkce_verifier_secret TEXT NOT NULL,

  next_url VARCHAR(256),

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE sessions (
  id TEXT NOT NULL PRIMARY KEY DEFAULT (
    lower(hex(randomblob(4))) || '-' ||
    lower(hex(randomblob(2))) || '-4' ||
    substr(lower(hex(randomblob(2))), 2) || '-a' ||
    substr(lower(hex(randomblob(2))), 2) || '-6' ||
    substr(lower(hex(randomblob(6))), 2)
  ),

  user_id TEXT NOT NULL REFERENCES users(id),
  client_ip VARCHAR(64),
  user_agent VARCHAR(128),

  created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  expires_at TIMESTAMP NOT NULL
);

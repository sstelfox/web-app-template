CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE background_tasks (
  id UUID DEFAULT uuid_generate_v4() NOT NULL PRIMARY KEY,

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
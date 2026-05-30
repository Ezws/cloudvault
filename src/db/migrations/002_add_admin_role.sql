-- Add admin role support to users
-- The first registered user becomes the admin; everyone else defaults to non-admin.

ALTER TABLE users ADD COLUMN IF NOT EXISTS is_admin BOOLEAN NOT NULL DEFAULT FALSE;

-- Promote the earliest-created existing user to admin so the deployment retains
-- an administrator after this migration runs against an existing database.
UPDATE users
SET is_admin = TRUE
WHERE id = (SELECT id FROM users ORDER BY created_at ASC LIMIT 1)
  AND NOT EXISTS (SELECT 1 FROM users WHERE is_admin = TRUE);

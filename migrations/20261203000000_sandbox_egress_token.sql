-- A sandbox process's egress token (digest), issued with its session at enrollment and valid
-- while the row lives: shells keep it in their environment, so it is never rotated underneath
-- them. A re-enrolled process (a Modal sandbox woken as a new VM) gets a new one.
ALTER TABLE sandboxes ADD COLUMN egress_hash BYTEA UNIQUE;

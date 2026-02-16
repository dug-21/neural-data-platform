-- ops-008: Layer 0 Foundation — Extensions
-- Run order: 1st (no dependencies)
-- Idempotent: Yes (IF NOT EXISTS)

CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS vector;

-- Verification
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
    RAISE EXCEPTION 'timescaledb extension failed to install';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
    RAISE EXCEPTION 'vector extension failed to install';
  END IF;
  RAISE NOTICE 'NDP init [001]: Extensions installed — timescaledb, vector';
END $$;

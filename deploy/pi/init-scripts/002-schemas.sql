-- ops-008: Layer 0 Foundation — Schemas
-- Run order: 2nd (depends on extensions)
-- Idempotent: Yes (IF NOT EXISTS)

CREATE SCHEMA IF NOT EXISTS data_dictionary;
CREATE SCHEMA IF NOT EXISTS silver;
CREATE SCHEMA IF NOT EXISTS gold;
CREATE SCHEMA IF NOT EXISTS analytics;

DO $$ BEGIN
  RAISE NOTICE 'NDP init [002]: Schemas created — data_dictionary, silver, gold, analytics';
END $$;

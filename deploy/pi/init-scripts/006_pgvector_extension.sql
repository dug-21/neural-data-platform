-- Enable pgvector extension for intelligence layer
-- This script runs during TimescaleDB initialization
CREATE EXTENSION IF NOT EXISTS vector;

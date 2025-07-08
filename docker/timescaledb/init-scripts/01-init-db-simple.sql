-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Create a simple schema
CREATE SCHEMA IF NOT EXISTS neural_trader;

-- Basic test to confirm database is working
SELECT 'Neural Trader Database Initialized' as status;
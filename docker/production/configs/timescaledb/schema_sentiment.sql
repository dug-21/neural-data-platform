-- Reddit Sentiment Analysis Tables
CREATE TABLE IF NOT EXISTS reddit_sentiment (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    subreddit VARCHAR(50) NOT NULL,
    mentions INTEGER NOT NULL DEFAULT 0,
    sentiment_score DOUBLE PRECISION,
    bullish_count INTEGER DEFAULT 0,
    bearish_count INTEGER DEFAULT 0,
    total_posts INTEGER DEFAULT 0,
    trending_rank INTEGER,
    metadata JSONB
);

-- Convert to hypertable
SELECT create_hypertable('reddit_sentiment', 'time', if_not_exists => TRUE);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_reddit_sentiment_symbol_time ON reddit_sentiment (symbol, time DESC);
CREATE INDEX IF NOT EXISTS idx_reddit_sentiment_subreddit ON reddit_sentiment (subreddit, time DESC);

-- Social sentiment aggregates (1 hour)
CREATE MATERIALIZED VIEW IF NOT EXISTS reddit_sentiment_1h
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) AS bucket,
    symbol,
    AVG(sentiment_score) as avg_sentiment,
    SUM(mentions) as total_mentions,
    SUM(bullish_count) as bullish_total,
    SUM(bearish_count) as bearish_total
FROM reddit_sentiment
GROUP BY bucket, symbol
WITH NO DATA;

-- Refresh policy
SELECT add_continuous_aggregate_policy('reddit_sentiment_1h',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '30 minutes',
    if_not_exists => TRUE);

-- Grant permissions
GRANT ALL PRIVILEGES ON reddit_sentiment TO neural_trader;
GRANT SELECT ON reddit_sentiment_1h TO neural_trader_readonly;
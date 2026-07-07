CREATE TABLE gamble_tables (
    id UUID PRIMARY KEY,
    screen_id UUID NOT NULL REFERENCES screens(id) ON DELETE CASCADE,
    game TEXT NOT NULL,
    config JSONB NOT NULL,
    phase TEXT NOT NULL DEFAULT 'betting',
    phase_ends_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '20 seconds',
    dealer_hand JSONB NOT NULL DEFAULT '[]',
    round_no INT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE gamble_seats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    table_id UUID NOT NULL REFERENCES gamble_tables(id) ON DELETE CASCADE,
    round_no INT NOT NULL,
    participant_id UUID NOT NULL,
    participant_name TEXT NOT NULL,
    stake INT NOT NULL,
    hand JSONB NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'playing',
    payout INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (table_id, round_no, participant_id)
);

CREATE INDEX idx_gamble_seats_table_round ON gamble_seats(table_id, round_no);

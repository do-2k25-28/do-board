-- Before this constraint existed, a participant could place more than one
-- bet per session (the bug this migration fixes). Keep only their earliest
-- bet per session and drop the rest, so the unique index below can be
-- created.
DELETE FROM interaction_responses
WHERE id IN (
    SELECT id FROM (
        SELECT id,
               ROW_NUMBER() OVER (
                   PARTITION BY session_id, participant_id
                   ORDER BY created_at, id
               ) AS rn
        FROM interaction_responses
        WHERE (payload ->> 'kind') = 'bet'
    ) ranked
    WHERE rn > 1
);

-- A participant may place at most one bet per session - enforced at the DB
-- level (not just in the handler) so it holds even under concurrent
-- requests. Scoped to bet responses only: Poll/Drawing responses are
-- unaffected (a Drawing participant legitimately submits many strokes).
CREATE UNIQUE INDEX idx_bet_one_response_per_participant
ON interaction_responses (session_id, participant_id)
WHERE (payload ->> 'kind') = 'bet';

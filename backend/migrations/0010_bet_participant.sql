ALTER TABLE interaction_responses ADD COLUMN participant_id UUID NOT NULL DEFAULT gen_random_uuid();

CREATE INDEX idx_interaction_responses_participant ON interaction_responses(participant_id);

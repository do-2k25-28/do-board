-- Converts pre-existing Bet slides/sessions/responses from the old flat
-- shape ({question, options} / {option_index, stake}) to the new
-- market-based shape ({question, market, result} / {pick, stake}), so
-- existing data keeps deserializing after the Rust types changed.

UPDATE screens
SET slides = (
    SELECT COALESCE(jsonb_agg(
        CASE
            WHEN slide -> 'config' ->> 'type' = 'interactive'
             AND slide -> 'config' -> 'interaction' ->> 'kind' = 'bet'
             AND (slide -> 'config' -> 'interaction' ? 'options')
            THEN jsonb_set(
                slide,
                '{config,interaction}',
                jsonb_build_object(
                    'kind', 'bet',
                    'question', slide -> 'config' -> 'interaction' -> 'question',
                    'market', jsonb_build_object(
                        'type', 'options',
                        'options', slide -> 'config' -> 'interaction' -> 'options'
                    ),
                    'result', 'null'::jsonb
                )
            )
            ELSE slide
        END
    ), '[]'::jsonb)
    FROM jsonb_array_elements(slides) AS slide
)
WHERE slides IS NOT NULL;

UPDATE interaction_sessions
SET config = jsonb_build_object(
    'kind', 'bet',
    'question', config -> 'question',
    'market', jsonb_build_object(
        'type', 'options',
        'options', config -> 'options'
    ),
    'result', 'null'::jsonb
)
WHERE kind = 'bet' AND config ? 'options';

UPDATE interaction_responses r
SET payload = jsonb_build_object(
    'kind', 'bet',
    'pick', jsonb_build_object(
        'type', 'options',
        'option_index', r.payload -> 'option_index'
    ),
    'stake', r.payload -> 'stake'
)
FROM interaction_sessions s
WHERE r.session_id = s.id
  AND s.kind = 'bet'
  AND r.payload ? 'option_index';

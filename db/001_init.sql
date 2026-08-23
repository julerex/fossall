-- Schema for the Fossall five-letter word list.
-- Apply against database `fossall` on cluster `postgreputest`.
-- See docs/DATABASE.md.

CREATE SCHEMA IF NOT EXISTS words;

CREATE TABLE IF NOT EXISTS words.five_letter_words (
    word TEXT PRIMARY KEY
        CHECK (
            char_length(word) = 5
            AND word = lower(word)
            AND word ~ '^[a-z]+$'
        )
);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'fossall') THEN
        GRANT USAGE ON SCHEMA words TO fossall;
        GRANT SELECT, INSERT ON TABLE words.five_letter_words TO fossall;
    END IF;
END
$$;

DROP TABLE IF EXISTS prefixes;
CREATE TABLE prefixes (
    guild_id BIGINT PRIMARY KEY NOT NULL,
    prefix TEXT NOT NULL
);

CREATE INDEX prefix_index ON prefixes (prefix);
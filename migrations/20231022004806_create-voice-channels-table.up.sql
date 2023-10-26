-- Add up migration script here
DROP TABLE IF EXISTS voice_channels;
CREATE TABLE voice_channels (
    channel_id BIGINT PRIMARY KEY NOT NULL,
    guild_id BIGINT NOT NULL,
    channel_template TEXT NOT NULL
);

CREATE UNIQUE INDEX channel_id_index ON voice_channels (channel_id);
CREATE INDEX guild_id_index ON voice_channels (guild_id);
CREATE UNIQUE INDEX channel_and_guild_id_index ON voice_channels (channel_id, guild_id);
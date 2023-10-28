-- Add up migration script here
DROP TABLE IF EXISTS template_channels;
CREATE TABLE template_channels (
    channel_id BIGINT PRIMARY KEY NOT NULL,
    guild_id BIGINT NOT NULL,
    channel_template TEXT NOT NULL,
    next_child_number BIGSERIAL NOT NULL
);

CREATE UNIQUE INDEX template_channel_id_index ON template_channels (channel_id);
CREATE INDEX template_channels_guild_id_index ON template_channels (guild_id);
CREATE UNIQUE INDEX channel_and_guild_id_index ON template_channels (channel_id, guild_id);
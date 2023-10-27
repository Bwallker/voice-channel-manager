DROP TABLE IF EXISTS child_channels;
CREATE TABLE child_channels (
    child_id BIGINT PRIMARY KEY NOT NULL,
    parent_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL
);

CREATE UNIQUE INDEX child_id_index ON child_channels (child_id);
CREATE INDEX child_channels_guild_id_index ON child_channels (guild_id);
CREATE UNIQUE INDEX child_and_guild_id_index ON child_channels (child_id, guild_id);
CREATE INDEX parent_id_index ON child_channels (parent_id);
CREATE UNIQUE INDEX child_id_and_parent_id_index ON child_channels (child_id, parent_id);
CREATE UNIQUE INDEX parent_id_and_guild_id_index ON child_channels (parent_id, guild_id);
CREATE UNIQUE INDEX child_channels_row_index ON child_channels (child_id, parent_id, guild_id);

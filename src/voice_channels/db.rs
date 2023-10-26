use serenity::model::prelude::*;
use sqlx::{PgExecutor, query};
use eyre::{eyre, WrapErr, Result};

pub async fn get_template<'a>(executor: impl PgExecutor<'a>, guild_id: GuildId) -> Result<Option<String>> {
    query!(
        "SELECT channel_template FROM voice_channels WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .fetch_optional(executor)
    .await
    .wrap_err_with(|| eyre!("Getting template from database for server with id {guild_id} failed!"))
    .map(|row| row.map(|row| row.channel_template))
}

pub async fn set_template<'a>(executor: impl PgExecutor<'a>, channel_id: ChannelId, guild_id: GuildId, template: String) -> Result<()> {
    query!(
        "INSERT INTO voice_channels (channel_id, guild_id, channel_template) VALUES ($1, $2, $3) ON CONFLICT (channel_id) DO UPDATE SET channel_template = $3;",
        channel_id.0 as i64,
        guild_id.0 as i64,
        template
    ).execute(executor).await.wrap_err_with(|| eyre!("Setting template in database for server with id {guild_id} failed!")).map(|_| ())
}

pub async fn delete_template<'a>(executor: impl PgExecutor<'a>, guild_id: GuildId) -> Result<()> {
    query!(
        "DELETE FROM voice_channels WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Deleting template from database for server with id {guild_id} failed!"))
    .map(|_| ())
}
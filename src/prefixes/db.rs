#![allow(unused_imports)]
use serenity::prelude::*;

use serenity::model::id::GuildId;

use eyre::{eyre, Context, Result};
use sqlx::{query, Pool, Postgres};

pub async fn get_prefix<'a>(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
) -> Result<Option<String>> {
    query!(
        "SELECT prefix FROM prefixes WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .fetch_optional(executor)
    .await
    .wrap_err_with(|| eyre!("Getting prefix from database for server with id {guild_id} failed!"))
    .map(|row| row.map(|row| row.prefix))
}

pub async fn set_prefix<'a>(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
    prefix: String,
) -> Result<()> {
    query!(
        "INSERT INTO prefixes (guild_id, prefix) VALUES ($1, $2) ON CONFLICT (guild_id) DO UPDATE SET prefix = $2;",
        guild_id.0 as i64,
        prefix
    ).execute(executor).await.wrap_err_with(|| eyre!("Setting prefix in database for server with id {guild_id} failed!")).map(|_| ())
}

pub async fn delete_prefix<'a>(executor: &Pool<Postgres>, guild_id: GuildId) -> Result<()> {
    query!(
        "DELETE FROM prefixes WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Deleting prefix from database for server with id {guild_id} failed!"))
    .map(|_| ())
}

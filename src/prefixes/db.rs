#![allow(unused_imports)]
use eyre::{
    eyre,
    Context,
    Result,
};
use serenity::{
    model::id::GuildId,
    prelude::*,
};
use sqlx::{
    query,
    PgPool,
};

pub(crate) async fn get_prefix(executor: &PgPool, guild_id: GuildId,) -> Result<Option<String,>,> {
    query!(
        "SELECT prefix FROM prefixes WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .fetch_optional(executor,)
    .await
    .wrap_err_with(|| eyre!("Getting prefix from database for server with id {guild_id} failed!"),)
    .map(|row| row.map(|row| row.prefix,),)
}

pub(crate) async fn set_prefix(
    executor: &PgPool,
    guild_id: GuildId,
    prefix: String,
) -> Result<(),> {
    query!(
        "INSERT INTO prefixes (guild_id, prefix) VALUES ($1, $2) ON CONFLICT (guild_id) DO UPDATE \
         SET prefix = $2;",
        guild_id.0 as i64,
        prefix
    )
    .execute(executor,)
    .await
    .wrap_err_with(|| eyre!("Setting prefix in database for server with id {guild_id} failed!"),)
    .map(|_| (),)
}

pub(crate) async fn delete_prefix(executor: &PgPool, guild_id: GuildId,) -> Result<(),> {
    query!(
        "DELETE FROM prefixes WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .execute(executor,)
    .await
    .wrap_err_with(|| eyre!("Deleting prefix from database for server with id {guild_id} failed!"),)
    .map(|_| (),)
}

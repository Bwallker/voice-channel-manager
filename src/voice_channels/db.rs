use eyre::{eyre, Result, WrapErr};
use serenity::model::prelude::*;
use sqlx::{query, PgExecutor};
use tracing::info;

pub async fn get_template<'a>(
    executor: impl PgExecutor<'a>,
    guild_id: GuildId,
) -> Result<Option<String>> {
    query!(
        "SELECT channel_template FROM template_channels WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .fetch_optional(executor)
    .await
    .wrap_err_with(|| eyre!("Getting template from database for server with id {guild_id} failed!"))
    .map(|row| row.map(|row| row.channel_template))
}

pub async fn set_template<'a>(
    executor: impl PgExecutor<'a>,
    channel_id: ChannelId,
    guild_id: GuildId,
    template: String,
) -> Result<()> {
    query!(
        "INSERT INTO template_channels (channel_id, guild_id, channel_template) VALUES ($1, $2, $3) ON CONFLICT (channel_id) DO UPDATE SET channel_template = $3;",
        channel_id.0 as i64,
        guild_id.0 as i64,
        template
    ).execute(executor).await.wrap_err_with(|| eyre!("Setting template in database for server with id {guild_id} failed!")).map(|_| ())
}

pub async fn delete_template<'a>(executor: impl PgExecutor<'a>, guild_id: GuildId) -> Result<()> {
    query!(
        "DELETE FROM template_channels WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting template from database for server with id {guild_id} failed!")
    })
    .map(|_| ())
}

pub async fn register_child<'a>(
    executor: impl PgExecutor<'a>,
    guild_id: GuildId,
    parent_id: ChannelId,
    child_id: ChannelId,
) -> Result<()> {
    query!(
        "INSERT INTO child_channels (guild_id, parent_id, child_id) VALUES ($1, $2, $3) ON CONFLICT (parent_id, child_id) DO NOTHING;",
        guild_id.0 as i64,
        parent_id.0 as i64,
        child_id.0 as i64
    ).execute(executor).await.wrap_err_with(|| eyre!("Registering child channel in database for server with id {guild_id} failed!")).map(|_| ())
}

pub async fn delete_child<'a>(
    executor: impl PgExecutor<'a>,
    guild_id: GuildId,
    parent_id: ChannelId,
    child_id: ChannelId,
) -> Result<()> {
    query!(
        "DELETE FROM child_channels WHERE guild_id = $1 AND parent_id = $2 AND child_id = $3;",
        guild_id.0 as i64,
        parent_id.0 as i64,
        child_id.0 as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting child channel from database for server with id {guild_id} failed!")
    })
    .map(|_| ())
}

pub async fn count_children<'a>(
    executor: impl PgExecutor<'a>,
    guild_id: GuildId,
    parent_id: ChannelId,
) -> Result<u64> {
    let res = query!(
        "SELECT COUNT(*) FROM child_channels WHERE guild_id = $1 AND parent_id = $2;",
        guild_id.0 as i64,
        parent_id.0 as i64
    )
    .fetch_one(executor)
    .await
    .wrap_err_with(|| {
        eyre!("Counting child channels in database for server with id {guild_id} failed!")
    })?;
    Ok(res.count.expect("Record to be present.") as u64)
}

pub enum TemplatedChannel {
    Parent {
        parent_id: ChannelId,
        next_child_number: u64,
        total_children_number: u64,
    },
    Child {
        child_id: ChannelId,
        child_number: u64,
        total_children_number: u64,
    },
}

pub async fn get_all_channels<'a>(
    executor: impl PgExecutor<'a>,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<Vec<TemplatedChannel>> {
    let res = query!(
        "SELECT child_id, parent_id, count(parent_id), child_number FROM child_channels INNER JOIN template_channels ON parent_id = channel_id WHERE child_channels.guild_id = $1 AND (parent_id = $2 OR child_id = $2) GROUP BY child_id;",
        guild_id.0 as i64,
        channel_id.0 as i64
    )
    .fetch_all(executor)
    .await
    .wrap_err_with(|| {
        eyre!("Getting all child channels in database for server with id {guild_id} failed!")
    })?;
    Ok(res
        .into_iter()
        .map(|row| {
            let child_id = ChannelId(row.child_id as u64);
            let parent_id = ChannelId(row.parent_id as u64);
            let child_number = row.child_number as u64;
            let total_children_number = row.count.unwrap_or(0) as u64;
            let next_child_number = row.next_child_number as u64;
            if child_id == channel_id {
                TemplatedChannel::Child {
                    child_id,
                    child_number,
                    total_children_number,
                }
            } else {
                TemplatedChannel::Parent {
                    parent_id,
                    next_child_number,
                    total_children_number,
                }
            }
        })
        .collect())
}

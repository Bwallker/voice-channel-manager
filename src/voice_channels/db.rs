use crate::HashMap;
use core::hash::Hash;
use eyre::{eyre, Result, WrapErr};
use serenity::model::prelude::*;
use sqlx::{query, Pool, Postgres};
use std::hash::Hasher;
use tokio::join;
#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

#[allow(dead_code)]
pub async fn get_template(executor: &Pool<Postgres>, guild_id: GuildId) -> Result<Option<String>> {
    query!(
        "SELECT channel_template FROM template_channels WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .fetch_optional(executor)
    .await
    .wrap_err_with(|| eyre!("Getting template from database for server with id {guild_id} failed!"))
    .map(|row| row.map(|row| row.channel_template))
}

pub async fn set_template(
    executor: &Pool<Postgres>,
    channel_id: ChannelId,
    guild_id: GuildId,
    template: String,
) -> Result<()> {
    query!(
        "INSERT INTO template_channels (channel_id, guild_id, channel_template, next_child_number) VALUES ($1, $2, $3, 1) ON CONFLICT (channel_id) DO UPDATE SET channel_template = $3;",
        channel_id.0 as i64,
        guild_id.0 as i64,
        template
    ).execute(executor).await.wrap_err_with(|| eyre!("Setting template in database for server with id {guild_id} failed!")).map(|_| ())
}

pub async fn delete_template(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<()> {
    let q1 = query!(
        "DELETE FROM template_channels WHERE guild_id = $1 AND channel_id = $2;",
        guild_id.0 as i64,
        channel_id.0 as i64
    )
    .execute(executor);
    let q2 = query!(
        "DELETE FROM child_channels WHERE guild_id = $1 AND parent_id = $2;",
        guild_id.0 as i64,
        channel_id.0 as i64,
    )
    .execute(executor);
    let (f1, f2) = join!(q1, q2);
    let mut error = None;
    if let Err(e) = f1 {
        error = Some(eyre!(e).wrap_err(eyre!(
            "Deleting template from database for server with id {guild_id} failed!"
        )));
    }
    if let Err(e) = f2 {
        error = Some(error.take().unwrap_or(eyre!(e)).wrap_err(eyre!(
            "Deleting children from database for server with id {guild_id} failed!"
        )));
    }
    match error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

pub async fn register_child(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
    parent_id: ChannelId,
    child_id: ChannelId,
) -> Result<u64> {
    let mut transaction = executor
        .begin()
        .await
        .wrap_err_with(|| eyre!("Failed to start a transaction!"))?;

    info!("Parent ID {parent_id}, Guild ID: {guild_id}, Child ID: {child_id}");

    let row = query!(
        "UPDATE template_channels SET next_child_number = next_child_number + 1 WHERE channel_id = $1 AND guild_id = $2 RETURNING next_child_number - 1 AS child_number;", parent_id.0 as i64, guild_id.0 as i64
    ).fetch_one(&mut *transaction).await.wrap_err_with(|| eyre!("Updating next child number in database for server with id {guild_id} failed!"))?;

    info!(
        "Successfully updated next_child_number!: Result: {:?}",
        row.child_number
    );
    query!(
        "INSERT INTO child_channels (guild_id, parent_id, child_id, child_number) VALUES ($1, $2, $3, $4) ON CONFLICT (child_id) DO NOTHING;",
        guild_id.0 as i64,
        parent_id.0 as i64,
        child_id.0 as i64,
        row.child_number
    ).execute(&mut *transaction).await.wrap_err_with(|| eyre!("Registering child channel in database for server with id {guild_id} failed!")).map(|_| ())?;

    let child_number = row
        .child_number
        .ok_or_else(|| eyre!("Child number not present!"))?;

    transaction
        .commit()
        .await
        .wrap_err_with(|| eyre!("Failed to commit transaction!"))?;

    Ok(child_number as u64)
}

#[allow(dead_code)]
pub async fn delete_child(
    executor: &Pool<Postgres>,
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

pub async fn change_capacity(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
    channel_id: ChannelId,
    capacity: u64,
) -> Result<()> {
    query!(
        "UPDATE template_channels SET capacity = $3 WHERE guild_id = $1 AND channel_id = $2;",
        guild_id.0 as i64,
        channel_id.0 as i64,
        capacity as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Updating capacity in database for server with id {guild_id} failed!"))
    .map(|_| ())
}

#[derive(Debug, Clone)]
pub struct Child {
    pub child_id: ChannelId,
    pub child_number: u64,
    pub total_children_number: u64,
    pub template: String,
}

pub type Children = Vec<Child>;

#[derive(Debug, Clone)]
pub struct Parent {
    pub parent_id: ChannelId,
    pub total_children_number: u64,
    pub template: String,
    pub capacity: Option<u64>,
}

impl From<ChannelId> for Parent {
    fn from(parent_id: ChannelId) -> Self {
        Self {
            parent_id,
            total_children_number: 0,
            template: String::new(),
            capacity: None,
        }
    }
}

impl Hash for Parent {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.parent_id.hash(hasher)
    }
}

impl PartialEq for Parent {
    fn eq(&self, other: &Self) -> bool {
        self.parent_id == other.parent_id
    }
}

impl Eq for Parent {}

pub async fn get_all_children_of_parent(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<Option<(Parent, Children)>> {
    let res = query!(
        "SELECT child_id, child_number, next_child_number, channel_template, parent_id, capacity
        FROM child_channels
        JOIN template_channels
        ON parent_id = channel_id
        WHERE child_channels.guild_id = $1
        AND (
            parent_id = $2
            OR child_id = $2
        );",
        guild_id.0 as i64,
        channel_id.0 as i64
    )
    .fetch_all(executor)
    .await
    .wrap_err_with(|| {
        eyre!("Getting all child channels in database for server with id {guild_id} failed!")
    })?;

    info!("Result from get_all_channels: {res:#?}");

    let mut iter = res.into_iter().peekable();

    let Some(parent_row) = iter.peek() else {
        return Ok(None);
    };

    let parent_id = ChannelId(parent_row.parent_id as u64);

    let total_children_number = parent_row.next_child_number as u64 - 1;

    let template = parent_row.channel_template.to_owned();

    let capacity = parent_row.capacity.map(|v| v as u64);

    let parent = Parent {
        parent_id,
        total_children_number,
        template,
        capacity,
    };

    let children = iter
        .map(|row| {
            let child_id = ChannelId(row.child_id as u64);

            let child_number = row.child_number as u64;

            let total_children_number = row.next_child_number as u64 - 1;
            let template = row.channel_template;
            Child {
                child_id,
                child_number,
                total_children_number,
                template,
            }
        })
        .collect();

    Ok(Some((parent, children)))
}

pub async fn get_all_channels_in_guild(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
) -> Result<HashMap<Parent, Children>> {
    info!("Retrieving all channels in guild with ID `{guild_id}`!");
    let mut transaction = executor
        .begin()
        .await
        .wrap_err_with(|| eyre!("Failed to start a transaction!"))?;

    let res = query!(
        "SELECT channel_template, parent_id, child_id, child_number, next_child_number, capacity FROM template_channels JOIN child_channels ON template_channels.channel_id = child_channels.parent_id WHERE template_channels.guild_id = $1;",
        guild_id.0 as i64
    ).fetch_all(&mut *transaction).await.wrap_err_with(|| eyre!("Getting all channels in guild with ID `{guild_id}` failed!"))?;

    let mut parent_channels = HashMap::default();

    for row in res {
        let parent_id = ChannelId(row.parent_id as u64);
        let child_id = ChannelId(row.child_id as u64);
        let child_number = row.child_number as u64;
        let next_child_number = row.next_child_number as u64;
        let template = row.channel_template;
        let capacity = row.capacity.map(|v| v as u64);

        let parent = Parent {
            parent_id,
            total_children_number: next_child_number - 1,
            template: template.clone(),
            capacity,
        };

        let entry = parent_channels.entry(parent).or_insert_with(Vec::new);
        let child = Child {
            child_id,
            child_number,
            total_children_number: next_child_number,
            template,
        };

        entry.push(child);
    }

    Ok(parent_channels)
}

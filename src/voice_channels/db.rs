use crate::{HashMap, HashSet};
use core::hash::Hash;
use eyre::{eyre, Result, WrapErr};
use if_chain::if_chain;
use serenity::model::prelude::*;
use sqlx::{query, query_as_unchecked, Pool, Postgres};
use std::hash::Hasher;

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
    let mut transaction = executor
        .begin()
        .await
        .wrap_err_with(|| eyre!("Failed to start a transaction!"))?;
    let res1 = query!(
        "DELETE FROM template_channels WHERE guild_id = $1 AND channel_id = $2;",
        guild_id.0 as i64,
        channel_id.0 as i64
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting template from database for server with id {guild_id} failed!")
    })?;

    debug!("Finished deleting parent with id {channel_id} from database!");
    assert_eq!(res1.rows_affected(), 1);
    let res2 = query!(
        "DELETE FROM child_channels WHERE guild_id = $1 AND parent_id = $2;",
        guild_id.0 as i64,
        channel_id.0 as i64,
    )
    .execute(executor)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting child channels from database for server with id {guild_id} failed!")
    })?;
    debug!(
        "Finished deleting {} children of parent with id {channel_id} from database!",
        res2.rows_affected()
    );

    Ok(())
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

#[derive(Debug, Clone, Default)]
pub struct Child {
    pub child_id: ChannelId,
    pub parent_id: ChannelId,
    pub child_number: u64,
    pub total_children_number: u64,
    pub template: String,
}

impl Hash for Child {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.child_id.hash(hasher)
    }
}

impl PartialEq for Child {
    fn eq(&self, other: &Self) -> bool {
        self.child_id == other.child_id
    }
}

impl Eq for Child {}

pub type Children = HashSet<Child>;

#[derive(Debug, Clone, Default)]
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
#[derive(Debug)]
struct GetAllChildren {
    child_id: Option<i64>,
    child_number: Option<i64>,
    next_child_number: i64,
    channel_template: String,
    channel_id: i64,
    capacity: Option<i64>,
}

pub async fn get_all_children_of_parent(
    executor: &Pool<Postgres>,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<Option<(Parent, Children)>> {
    debug!("Guild id and channel id in get all children is: {guild_id}, {channel_id}");
    let res = query_as_unchecked!(
        GetAllChildren,
        "SELECT child_id, child_number, next_child_number, channel_template, channel_id, capacity
        FROM template_channels
        LEFT JOIN child_channels
        ON parent_id = channel_id
        WHERE
        (
          template_channels.guild_id = $1 
          OR child_channels.guild_id = $1
        )
        AND (
            channel_id = $2
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

    let parent_id = ChannelId(parent_row.channel_id as u64);

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
        .filter_map(|row| {
            let child_id = ChannelId(row.child_id? as u64);

            let child_number = row.child_number? as u64;

            let total_children_number = row.next_child_number as u64 - 1;
            let template = row.channel_template;
            let parent_id = ChannelId(row.channel_id as u64);
            Some(Child {
                child_id,
                child_number,
                total_children_number,
                template,
                parent_id,
            })
        })
        .collect();

    Ok(Some((parent, children)))
}

#[derive(Debug)]
struct GetAllChannels {
    channel_template: String,
    channel_id: i64,
    child_id: Option<i64>,
    child_number: Option<i64>,
    next_child_number: i64,
    capacity: Option<i64>,
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

    let res = query_as_unchecked!(
        GetAllChannels,
        "SELECT channel_template, channel_id, child_id, child_number, next_child_number, capacity FROM template_channels LEFT JOIN child_channels ON template_channels.channel_id = child_channels.parent_id WHERE template_channels.guild_id = $1;",
        guild_id.0 as i64
    ).fetch_all(&mut *transaction).await.wrap_err_with(|| eyre!("Getting all channels in guild with ID `{guild_id}` failed!"))?;

    let mut parent_channels = HashMap::default();

    for row in res {
        let parent_id = ChannelId(row.channel_id as u64);
        let child_id = row.child_id.map(|v| ChannelId(v as u64));
        let child_number = row.child_number.map(|v| v as u64);
        let next_child_number = row.next_child_number as u64;
        let template = row.channel_template;
        let capacity = row.capacity.map(|v| v as u64);

        let parent = Parent {
            parent_id,
            total_children_number: next_child_number - 1,
            template: template.clone(),
            capacity,
        };

        let children = parent_channels
            .entry(parent)
            .or_insert_with(HashSet::default);

        if_chain! {
            if let Some(child_id) = child_id;
            if let Some(child_number) = child_number;
            then {
                let child = Child {
                    parent_id,
                    child_id,
                    child_number,
                    total_children_number: next_child_number,
                    template,
                };
                children.insert(child);
            }
        }
    }

    Ok(parent_channels)
}

pub async fn update_next_child_number(
    executor: &Pool<Postgres>,
    parent_id: ChannelId,
    child_id: ChannelId,
) -> Result<()> {
    let res = query!(
        "
        WITH
            cn AS (
                SELECT child_number FROM child_channels WHERE child_id=$2
            ),
            next AS (
                SELECT COALESCE(MIN(child_number) + 1, 1) AS res
                FROM child_channels
                WHERE (parent_id = $1) AND
                    (child_number + 1 NOT IN
                        (
                        SELECT child_number FROM child_channels
                        )
                    )
            )
        UPDATE template_channels
        SET next_child_number = 
        CASE
            WHEN (SELECT * FROM next) = (SELECT * FROM cn) + 1 THEN (SELECT * FROM cn)
            ELSE (SELECT * FROM next)
        END
        WHERE channel_id = $1
    ",
        parent_id.0 as i64,
        child_id.0 as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Failed to update next_child_number for parent with id {parent_id}"))?;

    let rows_affected = res.rows_affected();

    assert_eq!(
        rows_affected, 1,
        "Sanity check failed: update_next_child_number updated {rows_affected} rows!"
    );

    Ok(())
}

pub async fn init_next_child_number(executor: &Pool<Postgres>) -> Result<()> {
    let rows_affected = query!(
        "
        WITH next AS (
            SELECT MIN(child_number) + 1 AS res, parent_id
            FROM child_channels
            GROUP BY parent_id
        )
        UPDATE template_channels
        SET next_child_number = (SELECT COALESCE(res, 1) FROM next RIGHT JOIN template_channels ON parent_id = channel_id)
    "
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Failed to initialize next_child_number!"))?
    .rows_affected();

    info!("Updated {rows_affected} rows in init_next_child_number!");
    Ok(())
}

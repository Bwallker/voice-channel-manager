use core::hash::Hash;
use std::hash::Hasher;

use eyre::{
    eyre,
    Result,
    WrapErr,
};
use if_chain::if_chain;
use serenity::model::prelude::*;
use sqlx::{
    query,
    PgPool,
};
#[allow(unused_imports)]
use tracing::{
    debug,
    error,
    info,
    trace,
    warn,
};

use crate::{
    DropExt,
    HashMap,
    HashSet,
};

#[allow(dead_code)]
pub(crate) async fn get_template(executor: &PgPool, guild_id: GuildId) -> Result<Option<String>> {
    query!(
        "SELECT channel_template FROM template_channels WHERE guild_id = $1;",
        guild_id.get() as i64
    )
    .fetch_optional(executor)
    .await
    .wrap_err_with(|| eyre!("Getting template from database for server with id {guild_id} failed!"))
    .map(|row| row.map(|row| row.channel_template))
}

pub(crate) async fn set_template(
    executor: &PgPool,
    channel_id: ChannelId,
    guild_id: GuildId,
    template: String,
) -> Result<()> {
    query!(
        "INSERT INTO template_channels (channel_id, guild_id, channel_template, \
         next_child_number) VALUES ($1, $2, $3, 1) ON CONFLICT (channel_id) DO UPDATE SET \
         channel_template = $3;",
        channel_id.get() as i64,
        guild_id.get() as i64,
        template
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Setting template in database for server with id {guild_id} failed!"))
    .map(|_| ())
}

pub(crate) async fn delete_template(
    executor: &PgPool,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<()> {
    let mut transaction = executor
        .begin()
        .await
        .wrap_err_with(|| eyre!("Failed to start a transaction!"))?;
    let res1 = query!(
        "DELETE FROM template_channels WHERE guild_id = $1 AND channel_id = $2;",
        guild_id.get() as i64,
        channel_id.get() as i64
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
        guild_id.get() as i64,
        channel_id.get() as i64,
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

pub(crate) async fn register_child(
    executor: &PgPool,
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
        "UPDATE template_channels SET next_child_number = next_child_number + 1 WHERE channel_id \
         = $1 AND guild_id = $2 RETURNING next_child_number - 1 AS child_number;",
        parent_id.get() as i64,
        guild_id.get() as i64
    )
    .fetch_one(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Updating next child number in database for server with id {guild_id} failed!")
    })?;

    info!(
        "Successfully updated next_child_number!: Result: {:?}",
        row.child_number
    );
    query!(
        "INSERT INTO child_channels (guild_id, parent_id, child_id, child_number) VALUES ($1, $2, \
         $3, $4) ON CONFLICT (child_id) DO NOTHING;",
        guild_id.get() as i64,
        parent_id.get() as i64,
        child_id.get() as i64,
        row.child_number
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Registering child channel in database for server with id {guild_id} failed!")
    })
    .map(|_| ())?;

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
pub(crate) async fn delete_child(
    executor: &PgPool,
    guild_id: GuildId,
    parent_id: ChannelId,
    child_id: ChannelId,
) -> Result<()> {
    query!(
        "DELETE FROM child_channels WHERE guild_id = $1 AND parent_id = $2 AND child_id = $3;",
        guild_id.get() as i64,
        parent_id.get() as i64,
        child_id.get() as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting child channel from database for server with id {guild_id} failed!")
    })
    .map(|_| ())
}

pub(crate) async fn change_capacity(
    executor: &PgPool,
    guild_id: GuildId,
    channel_id: ChannelId,
    capacity: u64,
) -> Result<()> {
    query!(
        "UPDATE template_channels SET capacity = $3 WHERE guild_id = $1 AND channel_id = $2;",
        guild_id.get() as i64,
        channel_id.get() as i64,
        capacity as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Updating capacity in database for server with id {guild_id} failed!"))
    .map(|_| ())
}

pub(crate) async fn clear_capacity(
    executor: &PgPool,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<()> {
    query!(
        "UPDATE template_channels SET capacity = NULL WHERE guild_id = $1 AND channel_id = $2;",
        guild_id.get() as i64,
        channel_id.get() as i64
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Clearing capacity in database for server with id {guild_id} failed!"))
    .map(|_| ())
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Child {
    pub(crate) id:                    ChannelId,
    pub(crate) number:                u64,
    pub(crate) total_children_number: u64,
    pub(crate) template:              String,
}

impl Hash for Child {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.id.hash(hasher);
    }
}

impl PartialEq for Child {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Child {}

pub(crate) type Children = HashSet<Child>;

#[derive(Debug, Clone, Default)]
pub(crate) struct Parent {
    pub(crate) id:       ChannelId,
    pub(crate) template: String,
    pub(crate) capacity: Option<u64>,
}

impl From<ChannelId> for Parent {
    fn from(parent_id: ChannelId) -> Self {
        Self {
            id:       parent_id,
            template: String::new(),
            capacity: None,
        }
    }
}

impl Hash for Parent {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.id.hash(hasher);
    }
}

impl PartialEq for Parent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Parent {}

pub(crate) async fn get_all_children_of_parent(
    executor: &PgPool,
    guild_id: GuildId,
    channels: &[i64],
) -> Result<Option<(Parent, Children)>> {
    debug!("Guild id and channel id in get all children is: {guild_id}, {channels:?}");
    let res = query!(
        r#"
        SELECT child_id as "child_id?", child_number as "child_number?", next_child_number, channel_template, channel_id, capacity
        FROM template_channels
        LEFT JOIN child_channels
        ON parent_id = channel_id
        WHERE
        (
          template_channels.guild_id = $1 
          OR child_channels.guild_id = $1
        )
        AND (
            channel_id = ANY($2)
            OR child_id = ANY($2)
        );
        "#,
        guild_id.get() as i64,
        channels
    )
    .fetch_all(executor,)
    .await
    .wrap_err_with(|| {
        eyre!("Getting all child channels in database for server with id {guild_id} failed!")
    },)?;

    info!("Result from get_all_channels: {res:#?}");

    let mut iter = res.into_iter().peekable();

    let Some(parent_row) = iter.peek() else {
        return Ok(None);
    };

    let parent_id = ChannelId::new(parent_row.channel_id as u64);

    let template = parent_row.channel_template.clone();

    let capacity = parent_row.capacity.map(|v| v as u64);

    let parent = Parent {
        id: parent_id,
        template,
        capacity,
    };

    let children = iter
        .filter_map(|row| {
            let child_id = ChannelId::new(row.child_id? as u64);

            let child_number = row.child_number? as u64;

            let total_children_number = row.next_child_number as u64 - 1;
            let template = row.channel_template;
            Some(Child {
                id: child_id,
                number: child_number,
                total_children_number,
                template,
            })
        })
        .collect();

    Ok(Some((parent, children)))
}
pub(crate) async fn get_all_channels_in_guild(
    executor: &PgPool,
    guild_id: GuildId,
) -> Result<HashMap<Parent, Children>> {
    info!("Retrieving all channels in guild with ID `{guild_id}`!");
    let mut transaction = executor
        .begin()
        .await
        .wrap_err_with(|| eyre!("Failed to start a transaction!"))?;

    let res = query!(
        r#"
        SELECT channel_template, channel_id, child_id as "child_id?", child_number as "child_number?", next_child_number, capacity 
        FROM template_channels
        LEFT JOIN child_channels ON template_channels.channel_id = child_channels.parent_id
        WHERE template_channels.guild_id = $1;
        "#,
        guild_id.get() as i64
    )
    .fetch_all(&mut *transaction,)
    .await
    .wrap_err_with(|| eyre!("Getting all channels in guild with ID `{guild_id}` failed!"),)?;

    let mut parent_channels = HashMap::default();

    for row in res {
        let parent_id = ChannelId::new(row.channel_id as u64);
        let child_id = row.child_id.map(|v| ChannelId::new(v as u64));
        let child_number = row.child_number.map(|v| v as u64);
        let next_child_number = row.next_child_number as u64;
        let template = row.channel_template;
        let capacity = row.capacity.map(|v| v as u64);

        let parent = Parent {
            id: parent_id,
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
                    id: child_id,
                    number: child_number,
                    total_children_number: next_child_number,
                    template,
                };
                children.insert(child).drop();
            }
        }
    }

    Ok(parent_channels)
}

pub(crate) async fn update_next_child_number(
    executor: &PgPool,
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
        parent_id.get() as i64,
        child_id.get() as i64
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

pub(crate) async fn init_next_child_number(executor: &PgPool) -> Result<()> {
    let rows_affected = query!(
        "
        WITH next AS (
            SELECT MIN(child_number) + 1 AS res, parent_id
            FROM child_channels
            GROUP BY parent_id
        )
        UPDATE template_channels
        SET next_child_number = 
        CASE
            WHEN EXISTS (SELECT * FROM next) THEN (SELECT COALESCE(res, 1) FROM next RIGHT JOIN \
                        template_channels ON parent_id = channel_id)
            ELSE 1
        END
    "
    )
    .execute(executor)
    .await
    .wrap_err_with(|| eyre!("Failed to initialize next_child_number!"))?
    .rows_affected();

    info!("Updated {rows_affected} rows in init_next_child_number!");
    Ok(())
}

pub(crate) async fn remove_dead_channels(
    executor: &PgPool,
    deleted_parents: &[i64],
    deleted_children: &[i64],
) -> Result<()> {
    let mut transaction = executor
        .begin()
        .await
        .wrap_err_with(|| eyre!("Failed to start a transaction!"))?;
    let rows_affected = query!(
        "DELETE FROM child_channels WHERE child_id = ANY($1);",
        deleted_children
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| eyre!("Failed to remove deleted children!"))?
    .rows_affected();

    debug!("Deleted {rows_affected} rows in remove_deleted_children!");

    let rows_affected = query!(
        "DELETE FROM template_channels WHERE channel_id = ANY($1);",
        deleted_parents
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| eyre!("Failed to remove deleted parents!"))?
    .rows_affected();

    debug!("Deleted {rows_affected} rows in remove_deleted_parents!");

    Ok(())
}

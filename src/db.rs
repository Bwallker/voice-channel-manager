use eyre::{eyre, Result, WrapErr};
#[allow(unused_imports)]
use serenity::model::prelude::*;
#[allow(unused_imports)]
use serenity::prelude::*;
#[allow(unused_imports)]
use sqlx::prelude::*;
use sqlx::{query, PgPool};
use tracing::info;

pub(crate) async fn clean_left_guild_from_db(executor: &PgPool, guild_id: GuildId) -> Result<()> {
    info!("Cleaning guild with ID `{guild_id}` from database!");

    let mut transaction = executor.begin().await.wrap_err_with(|| {
        eyre!("Starting transaction for deleting guild {guild_id} from database failed!")
    })?;
    query!(
        "DELETE FROM child_channels WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting child channels from database for guild with ID `{guild_id}` failed!")
    })
    .map(|res| {
        info!(
            "Finished deleting {} rows from child_channels",
            res.rows_affected()
        );
    })?;

    query!(
        "DELETE FROM template_channels WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting template channels from database for guild with ID `{guild_id}` failed!")
    })
    .map(|res| {
        info!(
            "Finished deleting {} rows from template_channels",
            res.rows_affected()
        );
    })?;

    query!(
        "DELETE FROM prefixes WHERE guild_id = $1;",
        guild_id.0 as i64
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting prefixes from database for guild with ID `{guild_id}` failed!")
    })
    .map(|res| {
        info!(
            "Finished deleting {} rows from prefixes",
            res.rows_affected()
        );
    })?;

    transaction.commit().await.wrap_err_with(|| {
        eyre!("Committing transaction for deleting guild {guild_id} from database failed!")
    })?;

    info!("Finished removing guild with ID `{guild_id}` from database!");

    Ok(())
}

pub(crate) async fn clean_inactive_guilds_from_db(
    executor: &PgPool,
    guilds_to_keep: &[i64],
) -> Result<()> {
    info!("Cleaning inactive guilds from DB!");
    let mut transaction = executor.begin().await.wrap_err_with(|| {
        eyre!("Starting transaction for deleting child channels from database failed!")
    })?;

    query!(
        "DELETE FROM child_channels WHERE NOT guild_id = ANY($1);",
        guilds_to_keep
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting child channels from database for inactive guilds failed! Active guild ids were {guilds_to_keep:?}!")
    }).map(|res| info!("Finished deleting {} rows from child_channels", res.rows_affected()))?;

    query!(
        "DELETE FROM template_channels WHERE NOT guild_id = ANY($1);",
        guilds_to_keep
    )
    .execute(&mut *transaction)
    .await
    .wrap_err_with(|| {
        eyre!("Deleting template channels from database for inactive guilds failed! Active guild ids were {guilds_to_keep:?}!")
    })
    .map(|res| {
        info!(
            "Finished deleting {} rows from template_channels",
            res.rows_affected()
        );
    })?;

    query!("DELETE FROM prefixes WHERE NOT guild_id = ANY($1);", guilds_to_keep)
        .execute(&mut *transaction)
        .await
        .wrap_err_with(|| {
            eyre!("Deleting prefixes from database for inactive guilds failed! Active guild ids were {guilds_to_keep:?}!")
        })
        .map(|res| {
            info!(
                "Finished deleting {} rows from prefixes",
                res.rows_affected()
            );
        })?;

    transaction.commit().await.wrap_err_with(|| {
        eyre!("Committing transaction for deleting inactive guilds from database failed!")
    })?;

    info!("Finished removing inactive guilds!");

    Ok(())
}

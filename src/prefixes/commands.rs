use serenity::model::prelude::*;
use serenity::prelude::*;

use eyre::{eyre, WrapErr};

use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};

use crate::{command_parser::parse_command, get_db_handle, DBConnection};

#[command]
#[description("Changes the prefix for the server.")]
#[usage("<prefix>/change_prefix <new prefix>")]
#[example("vc/change_prefix !")]
#[num_args(1)]
#[required_permissions("MANAGE_GUILD")]
async fn change_prefix(ctx: &Context, msg: &Message) -> CommandResult {
    let command = parse_command(ctx, msg)
        .wrap_err_with(|| eyre!("Failed to parse change_prefix command!"))?;
    let new_prefix = command
        .get_nth_arg(0)
        .ok_or_else(|| eyre!("No prefix provided!"))?;
    super::db::set_prefix(
        &get_db_handle(&ctx).await,
        msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?,
        new_prefix.to_string(),
    )
    .await
    .wrap_err_with(|| eyre!("Failed to set prefix!"))?;
    msg.channel_id
        .say(
            &ctx.http,
            format!("Prefix successfully changed to {new_prefix}!"),
        )
        .await
        .wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[command]
#[description("Resets the prefix to the default value for the server.")]
#[usage("<prefix>/reset_prefix")]
#[example("vc/reset_prefix")]
#[num_args(0)]
#[required_permissions("MANAGE_GUILD")]
async fn reset_prefix(ctx: &Context, msg: &Message) -> CommandResult {
    let _command = parse_command(ctx, msg)
        .wrap_err_with(|| eyre!("Failed to parse change_prefix command!"))?;

    super::db::delete_prefix(
        &get_db_handle(&ctx).await,
        msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?,
    )
    .await
    .wrap_err_with(|| eyre!("Failed to reset prefix!"))?;
    msg.channel_id
        .say(&ctx.http, "Prefix successfully reset!")
        .await
        .wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[group("Prefixes")]
#[commands(change_prefix, reset_prefix)]
struct Prefixes;

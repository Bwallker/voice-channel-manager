use eyre::{
  eyre,
  WrapErr,
};
use serenity::{
  framework::standard::{
    macros::{
      command,
      group,
    },
    Args,
    CommandResult,
  },
  model::prelude::*,
  prelude::*,
};
use tracing::{
  info,
  trace_span,
  Instrument,
};

use crate::{
  get_db_handle,
  DropExt,
};

#[command]
#[description("Changes the prefix for the server.")]
#[usage("<prefix>/change_prefix <new prefix>")]
#[example("vc/change_prefix !")]
#[aliases("set_prefix")]
#[only_in(guild)]
#[num_args(1)]
#[required_permissions("MANAGE_GUILD")]
async fn change_prefix(ctx: &Context, msg: &Message, mut args: Args,) -> CommandResult {
  let span = trace_span!("change_prefix span");
  async move {
    args.trimmed().quoted().drop();
    let new_prefix = args
      .current()
      .ok_or_else(|| eyre!("No prefix provided!"),)?;
    info!("new_prefix: `{new_prefix}`");
    super::db::set_prefix(
      &get_db_handle(ctx,).await,
      msg
        .guild_id
        .ok_or_else(|| eyre!("No guild id provided!"),)?,
      new_prefix.to_string(),
    )
    .await
    .wrap_err_with(|| eyre!("Failed to set prefix!"),)?;
    msg
      .channel_id
      .say(
        &ctx.http,
        format!(
          "{}: Prefix successfully changed to `{new_prefix}`!",
          msg.author.mention()
        ),
      )
      .await
      .wrap_err_with(|| "Failed to send message!",)?
      .drop();

    Ok((),)
  }
  .instrument(span,)
  .await
}

#[command]
#[description("Resets the prefix to the default value for the server.")]
#[usage("<prefix>/reset_prefix")]
#[example("vc/reset_prefix")]
#[only_in(guild)]
#[num_args(0)]
#[required_permissions("MANAGE_GUILD")]
async fn reset_prefix(ctx: &Context, msg: &Message, _args: Args,) -> CommandResult {
  let span = trace_span!("reset_prefix span");
  async move {
    super::db::delete_prefix(
      &get_db_handle(ctx,).await,
      msg
        .guild_id
        .ok_or_else(|| eyre!("No guild id provided!"),)?,
    )
    .await
    .wrap_err_with(|| eyre!("Failed to reset prefix!"),)?;
    msg
      .channel_id
      .say(
        &ctx.http,
        format!("{}: Prefix successfully reset!", msg.author.mention()),
      )
      .await
      .wrap_err_with(|| "Failed to send message!",)?
      .drop();

    Ok((),)
  }
  .instrument(span,)
  .await
}

#[group("Prefixes")]
#[commands(change_prefix, reset_prefix)]
struct Prefixes;

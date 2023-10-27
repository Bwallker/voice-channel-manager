use serenity::json::JsonMap;
use serenity::model::prelude::*;
use serenity::prelude::*;

use eyre::{eyre, WrapErr};

use serde_json::Number;

use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};
use sqlx::types::JsonValue;

use crate::{command_parser::parse_command, get_db_handle};

#[command]
#[description("Alters the template for a template channel.")]
#[usage("<prefix>/alter_channel <channel id> <new template>")]
#[example("vc/alter_channel 123456789 \"Gaming channel number {#}\"")]
#[num_args(2)]
#[required_permissions("MANAGE_CHANNELS")]
async fn alter_channel(ctx: &Context, msg: &Message) -> CommandResult {
    let command = parse_command(ctx, msg)
        .wrap_err_with(|| eyre!("Failed to parse alter_channel command!"))?;
    let channel_id = command
        .get_nth_arg(0)
        .ok_or_else(|| eyre!("No channel id provided!"))?;
    let new_template = command
        .get_nth_arg(1)
        .ok_or_else(|| eyre!("No template provided!"))?;

    super::db::set_template(
        &get_db_handle(&ctx).await,
        ChannelId(
            channel_id
                .parse::<u64>()
                .wrap_err_with(|| eyre!("Failed to parse channel id!"))?,
        ),
        msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?,
        new_template.to_string(),
    )
    .await
    .wrap_err_with(|| eyre!("Failed to set template!"))?;
    msg.channel_id
        .say(
            &ctx.http,
            format!("Template successfully changed to `{new_template}`!"),
        )
        .await
        .wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[command]
#[description("Creates a new template channel with a name and a template.")]
#[usage("<prefix>/create_channel <channel name> <template>")]
#[example("vc/create_channel \"Create a gaming channel!\" \"Gaming channel number {#}\"")]
#[num_args(2)]
#[required_permissions("MANAGE_CHANNELS")]
async fn create_channel(ctx: &Context, msg: &Message) -> CommandResult {
    let command = parse_command(ctx, msg)
        .wrap_err_with(|| eyre!("Failed to parse create_channel command!"))?;

    let channel_name = command
        .get_nth_arg(0)
        .ok_or_else(|| eyre!("No channel name provided!"))?;

    let template = command
        .get_nth_arg(1)
        .ok_or_else(|| eyre!("No template provided!"))?;
    let mut options = JsonMap::new();
    options.insert(
        "name".to_string(),
        JsonValue::String(channel_name.to_string()),
    );
    options.insert("type".to_string(), JsonValue::Number(Number::from(2)));
    let channel = ctx
        .http
        .create_channel(
            msg.guild_id
                .ok_or_else(|| eyre!("No guild id provided!"))?
                .0,
            &options,
            Some("Creating a new voice channel"),
        )
        .await
        .wrap_err_with(|| eyre!("Failed to create voice channel!"))?;

    super::db::set_template(
        &get_db_handle(&ctx).await,
        channel.id,
        msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?,
        template.to_string(),
    )
    .await
    .wrap_err_with(|| eyre!("Failed to create template!"))?;
    msg.channel_id
        .say(&ctx.http, format!("Channel successfully created with name `{channel_name}` and template `{template}`!"))
        .await.wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[command]
#[description("Deletes a template channel .")]
#[usage("<prefix>/delete_channel <channel id>")]
#[example("vc/delete_channel 1234567890")]
#[num_args(1)]
#[required_permissions("MANAGE_CHANNELS")]
async fn delete_channel(ctx: &Context, msg: &Message) -> CommandResult {
    let command = parse_command(ctx, msg)
        .wrap_err_with(|| eyre!("Failed to parse delete_channel command!"))?;
    let channel_id = command
        .get_nth_arg(0)
        .ok_or_else(|| eyre!("No channel id provided!"))?;

    ctx.http
        .delete_channel(
            ChannelId(
                channel_id
                    .parse::<u64>()
                    .wrap_err_with(|| eyre!("Failed to parse channel id!"))?,
            )
            .0,
        )
        .await
        .wrap_err_with(|| eyre!("Failed to delete channel!"))?;
    super::db::delete_template(
        &get_db_handle(&ctx).await,
        msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?,
    )
    .await
    .wrap_err_with(|| eyre!("Failed to delete template!"))?;

    msg.channel_id
        .say(&ctx.http, "Channel successfully deleted!")
        .await
        .wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[group("Template channels")]
#[commands(alter_channel, create_channel, delete_channel)]
struct TemplateChannels;

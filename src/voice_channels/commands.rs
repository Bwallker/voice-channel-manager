use color_eyre::Section;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::{framework::standard::Args, json::JsonMap};

use eyre::{eyre, WrapErr};

use serde_json::Number;

use serenity::framework::standard::{
    macros::{command, group},
    CommandResult,
};
use sqlx::types::JsonValue;
#[allow(unused_imports)]
use tracing::{debug, info, trace, trace_span};

use crate::{events::delete_parent_and_children, get_db_handle, GuildChannels};

#[command]
#[description("Alters the template for a template channel.")]
#[usage("<prefix>/alter_template <channel id> <new template>")]
#[example("vc/alter_template 123456789 \"Gaming channel number {#}#\"")]
#[aliases("alter_channel", "alter_parent")]
#[only_in(guild)]
#[num_args(2)]
#[required_permissions("MANAGE_CHANNELS")]
async fn alter_template(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let channel_id = args
        .parse::<ChannelId>()
        .wrap_err_with(|| eyre!("Failed to parse channel id!"))
        .suggestion("Channel ID must be a valid integer.")?;
    args.advance();
    let mut new_template = args
        .current()
        .ok_or_else(|| eyre!("No template provided!"))?;
    if new_template.as_bytes()[0] == b'"' {
        new_template = &new_template[1..];
    }
    if new_template.as_bytes()[new_template.len() - 1] == b'"' {
        new_template = &new_template[..new_template.len() - 1];
    }

    info!("New template: {new_template}!");
    super::db::set_template(
        &get_db_handle(ctx).await,
        channel_id,
        msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?,
        new_template.to_string(),
    )
    .await
    .wrap_err_with(|| eyre!("Failed to set template!"))?;
    msg.channel_id
        .say(
            &ctx.http,
            format!(
                "{}: Template successfully changed to `{new_template}`!",
                msg.author.mention()
            ),
        )
        .await
        .wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[command]
#[description("Creates a new template channel with a name and a template.")]
#[usage("<prefix>/create_channel <channel name> <template>")]
#[example("vc/create_channel \"Create a gaming channel!\" \"Gaming channel number {#}#\"")]
#[only_in(guild)]
#[num_args(2)]
#[required_permissions("MANAGE_CHANNELS")]
async fn create_channel(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let span = trace_span!("create_channel span");
    span.in_scope(|| trace!("Entered create_channel!"));
    let channel_name = args
        .single_quoted::<String>()
        .wrap_err_with(|| eyre!("No channel name provided!"))?;
    span.in_scope(|| trace!("Channel name: {}!", channel_name));

    let template = args
        .quoted()
        .current()
        .ok_or_else(|| eyre!("No template provided!"))?;

    span.in_scope(|| trace!("Template: {}!", template));
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
        &get_db_handle(ctx).await,
        channel.id,
        msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?,
        template.to_string(),
    )
    .await
    .wrap_err_with(|| eyre!("Failed to create template!"))?;
    msg.channel_id
        .say(&ctx.http, format!("{}: Channel successfully created with name `{channel_name}` and template `{template}`!", msg.author.mention()))
        .await.wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[command]
#[description("Deletes a template channel.")]
#[usage("<prefix>/delete_channel <channel id>")]
#[example("vc/delete_channel 1234567890")]
#[aliases("remove_channel")]
#[only_in(guild)]
#[num_args(1)]
#[required_permissions("MANAGE_CHANNELS")]
async fn delete_channel(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let channel_id = args
        .single_quoted::<ChannelId>()
        .wrap_err_with(|| eyre!("Failed to parse channel id!"))
        .suggestion("Channel ID must be a valid integer.")?;

    let guild_id = msg.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?;

    let guild_channels_map = ctx
        .data
        .read()
        .await
        .get::<GuildChannels>()
        .unwrap()
        .clone();

    let entry = guild_channels_map
        .get(&guild_id)
        .ok_or_else(|| eyre!("No entry for guild ID {guild_id} in guild_channels_map"))?;
    let guild_map = entry.value();

    let entry = guild_map
        .get(&channel_id.into())
        .ok_or_else(|| eyre!("No entry for channel ID {channel_id} in guild_map"))?;

    let (parent, children) = entry.pair();
    delete_parent_and_children(ctx, guild_id, parent, children).await?;

    msg.channel_id
        .say(
            &ctx.http,
            format!("{}: Channel successfully deleted!", msg.author.mention()),
        )
        .await
        .wrap_err_with(|| "Failed to send message!")?;

    Ok(())
}

#[command]
#[description("Changes the capacity for generated channels for a template channel.")]
#[usage("<prefix>/change_capacity <channel id> <capacity>")]
#[example("vc/change_capacity 1234567890 42")]
#[aliases("change_cap", "set_cap", "set_capacity")]
#[only_in(guild)]
#[num_args(2)]
#[required_permissions("MANAGE_CHANNELS")]
async fn change_capacity(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    args.trimmed().quoted();
    let channel_id = args.single_quoted::<ChannelId>().wrap_err_with(|| {
        eyre!("Failed to parse channel id!").suggestion("Channel ID must be a valid integer.")
    })?;
    let capacity = args
        .single_quoted::<u64>()
        .wrap_err_with(|| eyre!("Failed to parse channel capacity!"))
        .suggestion("Channel capacity must be a valid integer.")?;
    let guild_id = msg
        .guild_id
        .ok_or_else(|| eyre!("Guild ID was missing!. This should be impossible!"))?;

    super::db::change_capacity(&get_db_handle(ctx).await, guild_id, channel_id, capacity)
        .await
        .wrap_err_with(|| eyre!("Failed at changing capacity!"))?;

    msg.channel_id
        .say(
            &ctx.http,
            format!(
                "{} - Successfully changed capacity to {capacity}!",
                msg.author.mention()
            ),
        )
        .await
        .wrap_err_with(|| eyre!("Failed to send message!"))?;
    info!("Changed capacity for channel with ID {channel_id} to {capacity}!");
    Ok(())
}

#[group("Template channels")]
#[commands(alter_template, create_channel, delete_channel, change_capacity)]
struct TemplateChannels;

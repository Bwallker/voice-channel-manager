use std::fmt::Write;

use eyre::{
    eyre,
    Result,
    WrapErr,
};
use poise::command;
use serde_json::Number;
use serenity::{
    json::JsonMap,
    model::prelude::*,
};
use sqlx::types::JsonValue;
use tracing::Instrument;
#[allow(unused_imports)]
use tracing::{
    debug,
    info,
    trace,
    trace_span,
};

use crate::{
    get_db_handle,
    Context,
    DropExt,
    util::CacheExt,
};

type CommandResult = Result<()>;

/// Alters the template for a template channel.
#[command(
    slash_command,
    category = "voice-channels",
    guild_only,
    aliases("alter_channel", "alter_parent"),
    required_permissions = "MANAGE_CHANNELS"
)]
pub(crate) async fn alter_template(
    ctx: Context<'_>,
    #[description = "The ID of the channel to alter"] channel_id: ChannelId,
    #[description = "The template to use"] new_template: String,
) -> CommandResult {
    let span = trace_span!("alter_template span");
    async move {
        info!("New template: {new_template}!");
        let guild_id = ctx.guild().unwrap().id;
        let parsed_template = super::parser::parse_template(&new_template)
            .wrap_err_with(|| eyre!("Failed to parse template!"))?;
        info!("Parsed template: {:#?}!", parsed_template);
        super::db::set_template(
            &get_db_handle(ctx.serenity_context()).await,
            channel_id,
            guild_id,
            new_template.clone(),
        )
        .await
        .wrap_err_with(|| eyre!("Failed to set template!"))?;
        ctx.channel_id()
            .say(
                &ctx.http(),
                format!(
                    "{}: Template successfully changed to `{new_template}`!",
                    ctx.author().mention()
                ),
            )
            .await
            .wrap_err_with(|| "Failed to send message!")?
            .drop();
        Ok(())
    }
    .instrument(span)
    .await
}
/// Creates a new template channel with a name and a template.
#[command(
    slash_command,
    category = "voice-channels",
    guild_only,
    required_permissions = "MANAGE_CHANNELS"
)]
pub(crate) async fn create_channel(
    ctx: Context<'_>,
    #[description = "The name of the channel to create"] channel_name: String,
    #[description = "The template to use"] template: String,
) -> CommandResult {
    let span = trace_span!("create_channel span");
    async move {
        trace!("Entered create_channel!");
        let guild_id = ctx.guild().unwrap().id;
        trace!("Template: {}!", template);
        let parsed_template = super::parser::parse_template(&template)
            .wrap_err_with(|| eyre!("Failed to parse template!"))?;
        trace!("Parsed template: {:#?}!", parsed_template);
        let mut options = JsonMap::new();
        options
            .insert(
                "name".to_string(),
                JsonValue::String(channel_name.to_string()),
            )
            .drop();
        options
            .insert("type".to_string(), JsonValue::Number(Number::from(2)))
            .drop();
        let channel = ctx
            .http()
            .create_channel(guild_id, &options, Some("Creating a new voice channel"))
            .await
            .wrap_err_with(|| eyre!("Failed to create voice channel!"))?;

        super::db::set_template(
            &get_db_handle(ctx.serenity_context()).await,
            channel.id,
            guild_id,
            template.to_string(),
        )
        .await
        .wrap_err_with(|| eyre!("Failed to create template!"))?;
        ctx.channel_id()
            .say(
                &ctx.http(),
                format!(
                    "{}: Channel successfully created with name `{channel_name}` and template \
                     `{template}`!",
                    ctx.author().mention()
                ),
            )
            .await
            .wrap_err_with(|| "Failed to send message!")?
            .drop();

        Ok(())
    }
    .instrument(span)
    .await
}
/// Changes the capacity for generated channels of a template channel.
#[command(
    slash_command,
    category = "voice-channels",
    guild_only,
    aliases("change_cap", "set_cap", "set_capacity"),
    required_permissions = "MANAGE_CHANNELS"
)]
pub(crate) async fn change_capacity(
    ctx: Context<'_>,
    #[description = "The ID of the channel whose capacity you want to change."]
    channel_id: ChannelId,
    #[description = "The new capacity to use."] capacity: u64,
) -> CommandResult {
    let span = trace_span!("change_capacity span");
    async move {
        let guild_id = ctx.guild().unwrap().id;

        super::db::change_capacity(
            &get_db_handle(ctx.serenity_context()).await,
            guild_id,
            channel_id,
            capacity,
        )
        .await
        .wrap_err_with(|| eyre!("Failed at changing capacity!"))?;

        ctx.channel_id()
            .say(
                &ctx.http(),
                format!(
                    "{} - Successfully changed capacity to {capacity}!",
                    ctx.author().mention()
                ),
            )
            .await
            .wrap_err_with(|| eyre!("Failed to send message!"))?
            .drop();
        info!("Changed capacity for channel with ID {channel_id} to {capacity}!");
        Ok(())
    }
    .instrument(span)
    .await
}
/// Clears the set capacity for generated channels of a template channel.
#[command(
    slash_command,
    category = "voice-channels",
    guild_only,
    aliases("clear_cap"),
    required_permissions = "MANAGE_CHANNELS"
)]
pub(crate) async fn clear_capacity(
    ctx: Context<'_>,
    #[description = "The ID of the channel whose capacity you want to delete."]
    channel_id: ChannelId,
) -> CommandResult {
    let span = trace_span!("clear_capacity span");
    async move {
        let guild_id = ctx.guild().unwrap().id;

        super::db::clear_capacity(
            &get_db_handle(ctx.serenity_context()).await,
            guild_id,
            channel_id,
        )
        .await
        .wrap_err_with(|| eyre!("Failed at clearing capacity!"))?;

        ctx.channel_id()
            .say(
                &ctx.http(),
                format!(
                    "{} - Successfully cleared capacity for channel with ID {channel_id}!",
                    ctx.author().mention()
                ),
            )
            .await
            .wrap_err_with(|| eyre!("Failed to send message!"))?
            .drop();
        info!("Cleared capacity for channel with ID {channel_id}!");
        Ok(())
    }
    .instrument(span)
    .await
}
/// Lists all template channels and their children in your guild.
#[command(
    slash_command,
    category = "voice-channels",
    guild_only,
    aliases(
        "list_template",
        "list_templates",
        "list_template_channel",
        "list",
        "list_channel",
        "list_channels"
    ),
    required_permissions = "MANAGE_CHANNELS"
)]
pub(crate) async fn list_template_channels(ctx: Context<'_>) -> CommandResult {
    let span = trace_span!("list_template_channels span");
    async move {
        let guild_id = ctx.guild().unwrap().id;
        let all_channels = super::db::get_all_channels_in_guild(
            &get_db_handle(ctx.serenity_context()).await,
            guild_id,
        )
        .await
        .wrap_err_with(|| {
            eyre!("Failed to get all channels in guild in list_template_channels!")
        })?;
        let mut message = format!("{}:\n`", ctx.author().mention(),);
        for (parent_number, (parent, children)) in (1..=all_channels.len()).zip(&all_channels) {
            let channel = ctx.cache().guild_channel(guild_id, parent.id)?;
            let parent_name = channel.name();
            writeln!(message, "\tParent {parent_number}: \"{parent_name}\"")
                .wrap_err_with(|| eyre!("Failed to write parent name to message!"))?;
            for child in children {
                let channel = ctx.cache().guild_channel(guild_id, parent.id)?;
                let child_number = child.number;
                let child_name = channel.name();
                writeln!(message, "\t\tChild {child_number}: \"{child_name}\"")
                    .wrap_err_with(|| eyre!("Failed to write child name to message!"))?;
            }
        }
        message.push('`');
        ctx.channel_id()
            .say(&ctx.http(), message)
            .await
            .wrap_err_with(|| eyre!("Failed to send message!"))?
            .drop();
        Ok(())
    }
    .instrument(span)
    .await
}

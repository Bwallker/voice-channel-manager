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
use tracing::Instrument;
#[allow(unused_imports)]
use tracing::{debug, info, trace, trace_span};

use crate::get_db_handle;

#[command]
#[description("Alters the template for a template channel.")]
#[usage("<prefix>/alter_template <channel id> <new template>")]
#[example("vc/alter_template 123456789 \"Gaming channel number {#}#\"")]
#[aliases("alter_channel", "alter_parent")]
#[only_in(guild)]
#[num_args(2)]
#[required_permissions("MANAGE_CHANNELS")]
async fn alter_template(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let span = trace_span!("alter_template span");
    async move {
        args.trimmed().quoted();
        let channel_id = args
            .parse::<ChannelId>()
            .wrap_err_with(|| eyre!("Failed to parse channel id!"))
            .suggestion("Channel ID must be a valid integer.")?;
        args.advance();
        let new_template = args
            .quoted()
            .current()
            .ok_or_else(|| eyre!("No template provided!"))?;

        info!("New template: {new_template}!");
        let parsed_template = super::parser::parse_template(new_template)
            .wrap_err_with(|| eyre!("Failed to parse template!"))?;
        info!("Parsed template: {:#?}!", parsed_template);
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
    .instrument(span)
    .await
}

#[command]
#[description("Creates a new template channel with a name and a template.")]
#[usage("<prefix>/create_channel <channel name> <template>")]
#[example("vc/create_channel \"Create a gaming channel!\" \"Gaming channel number {#}#\"")]
#[only_in(guild)]
#[num_args(2)]
#[required_permissions("MANAGE_CHANNELS")]
async fn create_channel(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let span = trace_span!("create_channel span");
    async move {
        args.trimmed().quoted();
        trace!("Entered create_channel!");
        let channel_name = args
            .single_quoted::<String>()
            .wrap_err_with(|| eyre!("No channel name provided!"))?;
        trace!("Channel name: {}!", channel_name);

        let template = args
            .quoted()
            .current()
            .ok_or_else(|| eyre!("No template provided!"))?;

        trace!("Template: {}!", template);
        let parsed_template = super::parser::parse_template(template)
            .wrap_err_with(|| eyre!("Failed to parse template!"))?;
        trace!("Parsed template: {:#?}!", parsed_template);
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
    }.instrument(span).await
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
    let span = trace_span!("change_capacity span");
    async move {
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
    .instrument(span)
    .await
}

#[group("Template channels")]
#[commands(alter_template, create_channel, change_capacity)]
struct TemplateChannels;

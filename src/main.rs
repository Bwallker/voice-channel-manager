use std::{
    collections::HashSet,
    env::var,
    sync::{Arc, OnceLock},
};

use dotenvy::dotenv;
use eyre::{eyre, Result, WrapErr};
use sqlx::{Pool, Postgres};
use tokio::runtime::Builder;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::FmtSubscriber;
use tracing_subscriber::{filter::LevelFilter, fmt::time::UtcTime};

struct DBConnection;

impl TypeMapKey for DBConnection {
    type Value = Pool<Postgres>;
}

struct ClientID;

impl TypeMapKey for ClientID {
    type Value = UserId;
}

struct DefaultPrefix;

impl TypeMapKey for DefaultPrefix {
    type Value = Arc<str>;
}

pub fn get_client_id() -> UserId {
    static ID: OnceLock<UserId> = OnceLock::new();
    *ID.get_or_init(|| {
        var("DISCORD_CLIENT_ID")
            .wrap_err_with(|| eyre!("Reading discord client id environment variable failed!"))
            .unwrap()
            .parse()
            .wrap_err_with(|| eyre!("Parsing discord client id failed!"))
            .unwrap()
    })
}

pub fn default_prefix() -> &'static str {
    static PREFIX: OnceLock<&'static str> = OnceLock::new();
    PREFIX.get_or_init(|| {
        get_default_prefix()
            .map(|ok| ok.unwrap_or("vc/".into()))
            .wrap_err_with(|| eyre!("Retrieving default prefix failed!"))
            .unwrap()
            .leak()
    })
}

#[allow(unused_imports)]
use tracing_subscriber::prelude::*;

use serenity::{
    framework::{
        standard::{
            help_commands::with_embeds,
            macros::{help, hook},
            Args, CommandGroup, CommandResult, HelpOptions,
        },
        StandardFramework,
    },
    model::prelude::*,
    prelude::*,
};

mod command_parser;
mod events;
mod prefixes;
mod voice_channels;

fn main() -> Result<()> {
    color_eyre::install().expect("Installing color_eyre to not fail.");
    dotenv().wrap_err_with(|| eyre!("Initializing .env failed!"))?;
    FmtSubscriber::builder()
        .with_timer(UtcTime::rfc_3339())
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .with_regex(false)
                .parse("")
                .wrap_err_with(|| eyre!("Parsing tracing filter failed!"))?,
        )
        .try_init()
        .map_err(|e| eyre!(e))
        .wrap_err_with(|| eyre!("Initializing tracing failed!"))?;
    info!("Running tokio runtime...");
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .wrap_err_with(|| eyre!("Failed to start tokio runtime"))?
        .block_on(start())?;

    Ok(())
}

#[hook]
async fn prefix_hook(ctx: &Context, message: &Message) -> Option<String> {
    prefixes::db::get_prefix(
        ctx.data.read().await.get::<DBConnection>().unwrap(),
        message.guild_id.unwrap(),
    )
    .await
    .wrap_err_with(|| eyre!("Retrieving server prefix failed!"))
    .unwrap()
}

#[hook]
async fn unrecognized_command_hook(ctx: &Context, message: &Message, command: &str) {
    info!("Unrecognized command: {command}");
    message
        .channel_id
        .say(
            &ctx.http,
            format!("Sorry, I don't know what you mean by `{command}`. Try `help`!"),
        )
        .await
        .wrap_err_with(|| eyre!("Sending message failed!"))
        .unwrap();
}

#[help]
#[embed_success_colour(ORANGE)]
#[max_levenshtein_distance(5)]
#[indention_prefix("---")]

async fn help_handler(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = with_embeds(context, msg, args, help_options, groups, owners).await?;
    Ok(())
}

fn get_default_prefix() -> Result<Option<String>> {
    match var("DEFAULT_PREFIX") {
        Ok(prefix) => Ok(Some(prefix)),
        Err(e) => match e {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(not_unicode) => Err(eyre!(
                "Error: Reading default prefix environment variable contained invalid unicode: {not_unicode:?}"
            )),
        },
    }
}

async fn start() -> Result<()> {
    info!("Starting application...");
    let intents = GatewayIntents::all();
    let framework = StandardFramework::new()
        .configure(|c| {
            c.prefix(default_prefix())
                .dynamic_prefix(prefix_hook)
                .on_mention(Some(get_client_id()))
        })
        .unrecognised_command(unrecognized_command_hook)
        .help(&HELP_HANDLER)
        .group(&prefixes::commands::PREFIXES_GROUP);

    let mut client = Client::builder(
        &var("DISCORD_TOKEN")
            .wrap_err_with(|| eyre!("Reading discord token environment variable failed!"))?,
        intents,
    )
    .event_handler(events::VoiceChannelManagerEventHandler::new())
    .framework(framework)
    .await
    .wrap_err_with(|| eyre!("Initializing serenity client failed!"))?;

    client
        .start_autosharded()
        .await
        .wrap_err_with(|| eyre!("Starting serenity client failed!"))?;
    Ok(())
}

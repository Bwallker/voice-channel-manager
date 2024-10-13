//! Main file of the voice channel manager bot.

#![cfg_attr(feature = "nightly-features", feature(must_not_suspend))]
#![cfg_attr(feature = "nightly-features", warn(must_not_suspend))]
#![cfg_attr(
    feature = "nightly-features",
    feature(non_exhaustive_omitted_patterns_lint)
)]
#![cfg_attr(feature = "nightly-features", warn(non_exhaustive_omitted_patterns))]

use std::{
    env::var,
    fmt::Display,
    str::FromStr,
    sync::{
        Arc,
        LazyLock,
    },
};
/// Utility trait to drop a value. Semantically equivalent to `std::mem::drop`.
pub(crate) trait DropExt {
    /// Method version of `std::mem::drop`.
    fn drop(self);
}

impl<T> DropExt for T {
    fn drop(self) {}
}

#[allow(dead_code, reason = "Useful thing to keep around.")]
pub(crate) trait ResultDropExt {
    type NoOk;
    type NoErr;
    fn drop_ok(self) -> Self::NoOk;
    fn drop_err(self) -> Self::NoErr;
}

impl<T, E> ResultDropExt for Result<T, E> {
    type NoErr = Result<T, ()>;
    type NoOk = Result<(), E>;

    fn drop_ok(self) -> Self::NoOk {
        self.map(|_| ())
    }

    fn drop_err(self) -> Self::NoErr {
        self.map_err(|_| ())
    }
}

pub(crate) type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;
pub(crate) type HashSet<K> = rustc_hash::FxHashSet<K>;

use dotenvy::dotenv;
use events::{on_event, on_ready};
use eyre::{
    eyre,
    Report,
    Result,
    WrapErr,
};
use poise::{
    builtins::register_globally,
    FrameworkOptions,
};
use serenity::{
    all::{
        Context as SerenityContext, CreateAllowedMentions, GatewayIntents, GuildId, Mentionable, UserId, VoiceState
    },
    prelude::TypeMapKey, Client,
};
use sqlx::PgPool;
use tokio::{
    runtime::Builder,
    sync::RwLock,
};
#[allow(unused_imports)]
use tracing::{
    debug,
    error,
    event,
    info,
    trace,
    warn,
    Level,
};
use tracing::{
    trace_span,
    Instrument,
};
use tracing_error::ErrorLayer;
#[allow(unused_imports)]
use tracing_subscriber::prelude::*;
use tracing_subscriber::{
    fmt::{
        format::Pretty,
        time::UtcTime,
    },
    EnvFilter,
    FmtSubscriber,
};
use voice_channels::{commands::{alter_template, change_capacity, clear_capacity, create_channel, list_template_channels}, db::{
    Children,
    Parent,
}};

mod db;
mod events;
mod util;
mod voice_channels;

struct DBConnection;

impl TypeMapKey for DBConnection {
    type Value = PgPool;
}

struct ClientID;

impl TypeMapKey for ClientID {
    type Value = UserId;
}

struct GuildChannels;

impl TypeMapKey for GuildChannels {
    type Value = Arc<RwLock<HashMap<GuildId, Arc<RwLock<HashMap<Parent, Children>>>>>>;
}

struct VoiceStates;

impl TypeMapKey for VoiceStates {
    type Value = Arc<RwLock<HashMap<GuildId, Arc<RwLock<HashMap<UserId, VoiceState>>>>>>;
}

pub(crate) async fn get_db_handle(ctx: &SerenityContext) -> PgPool {
    ctx.data.read().await.get::<DBConnection>().unwrap().clone()
}

pub(crate) static CLIENT_ID: LazyLock<UserId> = LazyLock::new(|| {
    var("DISCORD_CLIENT_ID")
        .wrap_err_with(|| eyre!("Reading discord client id environment variable failed!"))
        .unwrap()
        .parse()
        .wrap_err_with(|| eyre!("Parsing discord client id failed!"))
        .unwrap()
});

pub(crate) type Context<'a> = poise::Context<'a, (), Report>;
pub(crate) type Framework = poise::Framework<(), Report>;
pub(crate) type FrameworkContext<'a> = poise::FrameworkContext<'a, (), Report>;
pub(crate) type FrameworkError<'a> = poise::FrameworkError<'a, (), Report>;

fn main() -> Result<()> {
    color_eyre::install().expect("Installing color_eyre to not fail.");
    if let Err(err) = dotenv() {
        if err.not_found() {
            eprintln!(
                "Not .env file was found! This is not necessarily a fatal error if you have all \
                 necessary environment variables configured in your environment."
            );
        } else {
            return Err(eyre!(err).wrap_err(eyre!("Parsing .env file failed!")));
        }
    }
    let rust_log = var("RUST_LOG");
    FmtSubscriber::builder()
        .with_timer(UtcTime::rfc_3339())
        .with_env_filter(
            EnvFilter::from_str(
                rust_log
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or("voice_channel_manager=debug,info"),
            )
            .wrap_err_with(|| {
                eyre!(
                    "Parsing tracing filter from environment variable `RUST_LOG` failed! \
                     RUST_LOG: {rust_log:?}"
                )
            })?,
        )
        .finish()
        .with(ErrorLayer::new(Pretty::default()))
        .try_init()
        .map_err(|e| eyre!(e))
        .wrap_err_with(|| eyre!("Initializing tracing failed!"))?;
    info!("Running tokio runtime...");
    trace!("TRACE LOG !!!");
    info!(
        "RUST_LOG = {}",
        var("RUST_LOG").wrap_err_with(|| eyre!("Reading RUST_LOG environment variable failed!"))?
    );
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .wrap_err_with(|| eyre!("Failed to start tokio runtime"))?
        .block_on(start().instrument(trace_span!("Start span")))?;

    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn on_error(error: FrameworkError<'_>) {
    if let FrameworkError::Setup { error, framework: _, data_about_bot: _, ctx: _, .. } = error {
        error!("Setup returned an error: {error}");
        return;
    }
    if let FrameworkError::EventHandler { error, ctx: _, event: _, framework: _, .. } = error {
        error!("Handling event failed: {error}");
        return;
    }
    let channel_id = match error.ctx() {
        | Some(ctx) => ctx.channel_id(),
        | None => {
            let FrameworkError::UnknownInteraction { interaction, ctx: _, framework: _, .. } = error else {
                unreachable!(
                    "We've already handled all the errors that can occur that don't contain a \
                     poise context"
                )
            };
            interaction.channel_id
        },
    };
    let mention = match error.ctx() {
        | Some(ctx) => ctx.author().mention(),
        | None => {
            let FrameworkError::UnknownInteraction { interaction, ctx: _, framework: _, .. } = error else {
                unreachable!(
                    "We've already handled all the errors that can occur that don't contain a \
                     poise context"
                )
            };
            interaction.user.mention()
        },
    };
    let http = error.serenity_context().http.clone();
    let to_send = match error {
        | FrameworkError::Command { error, ctx, .. } =>
            format!("Running command {} failed: {error}", ctx.command().name),
        | FrameworkError::SubcommandRequired { .. } => unreachable!("We don't use subcommands"),
        | FrameworkError::CommandPanic { payload, ctx, .. } => format!(
            "Running command {} panicked: {}",
            ctx.command().name,
            payload.as_deref().unwrap_or("<no-panic-text>")
        ),
        | FrameworkError::ArgumentParse {
            error, input, ctx, ..
        } => format!(
            "Parsing arguments for command {} failed: {error} for input: {}",
            ctx.command().name,
            input.as_deref().unwrap_or("<no-argument-text>")
        ),
        | FrameworkError::CommandStructureMismatch { description: _, ctx: _, .. } => unreachable!(
            "We should get a command structure mismatch since we register all our commands in our \
             ready handler."
        ),
        | FrameworkError::CooldownHit { remaining_cooldown: _, ctx: _, .. } =>
            unreachable!("We don't have any cooldowns on our commands."),
        | FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => format!(
            "Bot is missing permissions to run command {}: {missing_permissions}",
            ctx.command().name
        ),
        | FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
            ..
        } => format!(
            "User is missing permissions to run command {}: {}",
            ctx.command().name,
            missing_permissions
                .as_ref()
                .map_or::<&dyn Display, _>(&"<unknown-permissions>", |v| v)
        ),
        | FrameworkError::NotAnOwner { ctx, .. } => format!(
            "User is not an owner and tried to run command {}",
            ctx.command().name
        ),
        | FrameworkError::GuildOnly { ctx, .. } =>
            format!("Command {} can only be run in guilds", ctx.command().name),
        | FrameworkError::DmOnly { ctx, .. } =>
            format!("Command {} can only be run in DMs", ctx.command().name),
        | FrameworkError::NsfwOnly { ctx, .. } => format!(
            "Command {} can only be run in NSFW channels",
            ctx.command().name
        ),
        | FrameworkError::CommandCheckFailed { error, ctx, .. } => format!(
            "Check for running command {} failed: {}",
            ctx.command().name,
            error.unwrap_or_else(|| eyre!("Check function returned false!")),
        ),
        | FrameworkError::DynamicPrefix { error: _, ctx: _, msg: _, .. } => unreachable!("We don't use dynamic prefixes"),
        | FrameworkError::UnknownCommand { ctx: _, msg: _, prefix: _, msg_content: _, framework: _, invocation_data: _, trigger: _, .. } =>
            unreachable!("We don't use prefix commands so we never encounter unknown commands"),
        | FrameworkError::UnknownInteraction { interaction, ctx: _, framework: _, .. } =>
            format!("Unknown interaction: {}", interaction.data.name),
        | _ => unreachable!("We've handled all of the errors that can occur"),
    };
    channel_id
        .say(http, format!("{mention} - Error: ") + &to_send)
        .await
        .wrap_err_with(|| eyre!("Sending message failed!"))
        .unwrap()
        .drop();
}

#[allow(clippy::unused_async)]
async fn on_before_command(ctx: Context<'_>) {
    let command_name = &ctx.command().name;
    info!("Dispatching command `{command_name}`");
}

#[allow(clippy::unused_async)]
async fn on_after_command(ctx: Context<'_>) {
    let command_name = &ctx.command().name;
    info!("Finished performing command `{command_name}`.");
}

async fn start() -> Result<()> {
    info!("Starting application...");
    let intents = GatewayIntents::all();
    let framework = Framework::builder()
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
                register_globally(ctx, &framework.options().commands)
                    .await
                    .wrap_err_with(|| eyre!("Failed to register commands!"))?;

                on_ready(ctx, ready, framework)
                    .instrument(trace_span!("Ready span"))
                    .await
            })
        })
        .options(FrameworkOptions {
            on_error: |error| Box::pin(on_error(error)),
            pre_command: |ctx| Box::pin(on_before_command(ctx)),
            post_command: |ctx| Box::pin(on_after_command(ctx)),
            command_check: None,
            skip_checks_for_owners: false,
            allowed_mentions: Some(
                CreateAllowedMentions::new()
                    .all_roles(true)
                    .all_users(true)
                    .everyone(true),
            ),
            reply_callback: None,
            manual_cooldowns: false,
            require_cache_for_guild_check: true,
            event_handler: |ctx, event, framework, _user_data| {
                Box::pin(on_event(ctx, event, framework))
            },
            commands: vec![
                alter_template(),
                create_channel(),
                change_capacity(),
                clear_capacity(),
                list_template_channels(),
            ],
            ..Default::default()
        })
        .build();

    let mut client = Client::builder(
        &var("DISCORD_TOKEN")
            .wrap_err_with(|| eyre!("Reading discord token environment variable failed!"))?,
        intents,
    )
    .framework(framework)
    .await
    .wrap_err_with(|| eyre!("Initializing serenity client failed!"))?;

    info!("Running discord client!");

    client
        .start_autosharded()
        .await
        .wrap_err_with(|| eyre!("Starting serenity client failed!"))?;
    Ok(())
}

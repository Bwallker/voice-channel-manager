use std::{
    collections::HashSet as SlowSet,
    env::var,
    sync::{Arc, OnceLock},
};

pub type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;
pub type HashSet<K> = rustc_hash::FxHashSet<K>;

use dotenvy::dotenv;
use eyre::{eyre, Result, WrapErr};
use sqlx::{Pool, Postgres};
use tokio::runtime::Builder;
#[allow(unused_imports)]
use tracing::{debug, error, event, info, trace, warn, Level};
use tracing_subscriber::fmt::time::UtcTime;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::FmtSubscriber;

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

struct GuildChannels;

impl TypeMapKey for GuildChannels {
    type Value = Arc<RwLock<HashMap<GuildId, Arc<RwLock<HashMap<Parent, Children>>>>>>;
}

pub async fn get_db_handle(ctx: &Context) -> Pool<Postgres> {
    ctx.data.read().await.get::<DBConnection>().unwrap().clone()
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
            Args, CommandError, CommandGroup, CommandResult, DispatchError, HelpOptions,
        },
        StandardFramework,
    },
    model::prelude::*,
    prelude::*,
};
use voice_channels::db::{Children, Parent};

mod events;
mod prefixes;
mod util;
mod voice_channels;

fn main() -> Result<()> {
    color_eyre::install().expect("Installing color_eyre to not fail.");
    dotenv().wrap_err_with(|| eyre!("Initializing .env failed!"))?;
    FmtSubscriber::builder()
        .with_timer(UtcTime::rfc_3339())
        .with_env_filter(
            EnvFilter::builder()
                .with_regex(true)
                .try_from_env()
                .wrap_err_with(|| {
                    eyre!("Parsing tracing filter from environment variable `RUST_LOG` failed!")
                })?,
        )
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
        .block_on(start())?;

    Ok(())
}

#[hook]
async fn prefix_hook(ctx: &Context, message: &Message) -> Option<String> {
    prefixes::db::get_prefix(&get_db_handle(ctx).await, message.guild_id.unwrap())
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
            format!(
                "{}: Sorry, I don't know what you mean by `{command}`. Try `help`!",
                message.author.mention()
            ),
        )
        .await
        .wrap_err_with(|| eyre!("Sending message failed!"))
        .unwrap();
}

#[hook]
async fn on_dispatch_error_hook(
    ctx: &Context,
    message: &Message,
    error: DispatchError,
    command: &str,
) {
    info!("Dispatch error: {error:#?}");
    let to_send = match error {
        DispatchError::CheckFailed(check_name, reason) => format!("Running command `{command}` failed!. Check `{check_name}` rejected you with reason: `{reason}`"),
        DispatchError::Ratelimited(info) => format!("You've been ratelimited and are unable to send commands!: details: `{info:#?}`"),
        DispatchError::CommandDisabled => format!("The command `{command}` has been disabled!"),
        DispatchError::BlockedUser => "You've been blocked from using commands!".to_string(),
        DispatchError::BlockedGuild => "This guild has been blocked from using commands!".to_string(),
        DispatchError::BlockedChannel => "This channel has been blocked from using commands!".to_string(),
        DispatchError::OnlyForDM => format!("Command `{command}` can only be used in DMs!"),
        DispatchError::OnlyForGuilds => format!("Command `{command}` can only be used in guilds!"),
        DispatchError::OnlyForOwners => format!("Command `{command}` can only be used by the bot owner!"),
        DispatchError::LackingRole => format!("You lack the necessary role to use command `{command}!"),
        DispatchError::LackingPermissions(permissions) => format!("You lack the required permissions to use command `{command}`! Required permissions: `{permissions}`"),
        DispatchError::NotEnoughArguments { min, given } => format!("Not enough arguments provided for command `{command}`! Minimum arguments: `{min}`, given arguments: `{given}`"),
        DispatchError::TooManyArguments { max, given } => format!("Too many arguments provided for command `{command}`! Maximum arguments: `{max}`, given arguments: `{given}`"),
        err => format!("Running command `{command}` failed! Error: `{err:#?}`"),
    };
    message
        .channel_id
        .say(
            &ctx.http,
            format!("{} - Error: ", message.author.mention()) + &to_send,
        )
        .await
        .wrap_err_with(|| eyre!("Sending message failed!"))
        .unwrap();
}

#[hook]
async fn on_before_command_hook(_ctx: &Context, _msg: &Message, command_name: &str) -> bool {
    info!("Dispatching command `{command_name}`");
    true
}

#[hook]
async fn on_after_command_hook(
    ctx: &Context,
    msg: &Message,
    command_name: &str,
    res: Result<(), CommandError>,
) {
    info!("Finished performing command `{command_name}`.");
    if let Err(e) = res {
        error!("Error executing command `{command_name}`: `{e:?}`");
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    "{}: Error executing command: \n\n`{command_name}: \n\n{e:?}`",
                    msg.author.mention()
                ),
            )
            .await
            .wrap_err_with(|| eyre!("Failed to send message!"))
            .unwrap();
    }
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
    owners: SlowSet<UserId>,
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
        .configure(|c| c.prefix(default_prefix()).dynamic_prefix(prefix_hook))
        .unrecognised_command(unrecognized_command_hook)
        .on_dispatch_error(on_dispatch_error_hook)
        .before(on_before_command_hook)
        .after(on_after_command_hook)
        .help(&HELP_HANDLER)
        .group(&prefixes::commands::PREFIXES_GROUP)
        .group(&voice_channels::commands::TEMPLATECHANNELS_GROUP);

    let mut client = Client::builder(
        &var("DISCORD_TOKEN")
            .wrap_err_with(|| eyre!("Reading discord token environment variable failed!"))?,
        intents,
    )
    .raw_event_handler(events::VoiceChannelManagerEventHandler::new())
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

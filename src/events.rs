use std::{
    env::var,
    ops::Not,
    sync::{
        Arc,
        LazyLock,
    },
};

use color_eyre::Section;
use eyre::{
    eyre,
    Report,
    Result,
    WrapErr,
};
use futures::future::join_all;
use serde_json::{
    Map,
    Number,
    Value,
};
use serenity::{
    all::{
        ActivityData,
        ChannelId,
        Guild,
        GuildChannel,
        GuildId,
        Member,
        Message,
        OnlineStatus,
        Ready,
        UnavailableGuild,
        VoiceState,
    },
    client::{
        Context as SerenityContext,
        FullEvent,
    },
};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::RwLock;
use tracing::Instrument;
#[allow(unused_imports)]
use tracing::{
    debug,
    debug_span,
    error,
    info,
    info_span,
    trace,
    trace_span,
};
use voice_channels::db::{
    Child,
    Parent,
};

use crate::{
    db::{
        clean_inactive_guilds_from_db,
        clean_left_guild_from_db,
    },
    get_db_handle,
    util::{
        get_value,
        CacheExt,
    },
    voice_channels::{
        self,
        db::Children,
        parser::parse_template,
        updater::{
            SerenityContextWrapper,
            UpdaterContext,
        },
    },
    ClientID,
    DBConnection,
    DropExt,
    Framework,
    FrameworkContext,
    GuildChannels,
    HashMap,
    VoiceStates,
    CLIENT_ID,
};

pub(crate) async fn delete_parent_and_children(
    ctx: &SerenityContext,
    guild_id: GuildId,
    parent: &Parent,
    children: &Children,
) -> Result<()> {
    ctx.http
        .delete_channel(parent.id, Some("Deleting parent channel during cleanup."))
        .await
        .wrap_err_with(|| eyre!("Failed to delete channel!"))?
        .drop();
    let child_results = join_all(children.iter().map(|c| c.id.delete(&ctx.http))).await;
    let mut child_result = Option::<Report>::None;

    debug!("Child results: {child_results:#?}");

    for result in child_results.into_iter().filter_map(Result::err) {
        match child_result.take() {
            | Some(err) => {
                child_result = Some(err.with_error(|| result));
            },
            | None => {
                child_result = Some(eyre!("Deleting child failed: {result}"));
            },
        }
    }
    if let Some(err) = child_result {
        return Err(err);
    }

    voice_channels::db::delete_template(&get_db_handle(ctx).await, guild_id, parent.id)
        .await
        .wrap_err_with(|| eyre!("Failed to delete template!"))?;

    let guild_map = {
        let guild_channels_map = get_value::<GuildChannels>(&ctx.data).await;
        let guild_channels_lock = guild_channels_map.read().await;
        guild_channels_lock.get(&guild_id).unwrap().clone()
    };

    let mut guild_map_lock = guild_map.write().await;
    guild_map_lock
        .remove(parent)
        .ok_or_else(|| eyre!("Parent did not exist in map!"))?
        .drop();

    Ok(())
}

async fn on_channel_delete(
    ctx: &SerenityContext,
    channel: &GuildChannel,
    _messages: Option<&Vec<Message>>,
) -> Result<()> {
    info!("Channel deleted: {}", channel.id);
    let guild_id = channel.guild_id;
    let Some((parent, children)) = voice_channels::db::get_all_children_of_parent(
        &get_db_handle(ctx).await,
        guild_id,
        &[channel.id.get() as i64],
    )
    .await
    .wrap_err_with(|| eyre!("Retrieving voice channels failed!"))?
    else {
        return Ok(());
    };
    if channel.id == parent.id {
        delete_parent_and_children(ctx, guild_id, &parent, &children).await
    } else {
        let guild_map = {
            let guild_channels_map = get_value::<GuildChannels>(&ctx.data).await;
            let guild_channels_lock = guild_channels_map.read().await;
            guild_channels_lock.get(&guild_id).unwrap().clone()
        };

        let mut guild_map_lock = guild_map.write().await;
        let channel_set = guild_map_lock
            .get_mut(&parent)
            .ok_or_else(|| eyre!("Parent was not in map!"))?;
        let child = channel_set
            .get(&Child {
                id: channel.id,
                ..Default::default()
            })
            .ok_or_else(|| eyre!("Child was not in map!"))?
            .clone();
        channel_set.remove(&child).drop();
        let db_handle = get_db_handle(ctx).await;
        voice_channels::db::update_next_child_number(&db_handle, parent.id, child.id)
            .await
            .wrap_err_with(|| eyre!("Failed to update next_channel_number!"))?;
        voice_channels::db::delete_child(&db_handle, channel.guild_id, parent.id, channel.id).await
    }
}

#[allow(clippy::unused_async)]
async fn on_channel_update(
    _ctx: &SerenityContext,
    old: Option<&GuildChannel>,
    new: &GuildChannel,
) -> Result<()> {
    info!("Updating channel: {}", new.id);
    info!("Old: `{old:#?}`, new: `{new:#?}`");
    let name = new.name();
    info!("Channel name is `{name}`");

    Ok(())
}

pub(crate) async fn on_ready(
    ctx: &SerenityContext,
    ready: &Ready,
    _framework: &Framework,
) -> Result<()> {
    info!("{} is connected!", ready.user.name);

    let mut lock = ctx.data.write().await;
    lock.insert::<DBConnection>(
        PgPoolOptions::new()
            .max_connections(1024)
            .connect(
                &var("DATABASE_URL")
                    .wrap_err_with(|| eyre!("Reading database url environment variable failed!"))?,
            )
            .await
            .wrap_err_with(|| eyre!("Connecting to database failed!"))?,
    );
    lock.insert::<ClientID>(*LazyLock::force(&CLIENT_ID));
    lock.insert::<GuildChannels>(Arc::new(RwLock::new(HashMap::default())));
    lock.insert::<VoiceStates>(Arc::new(RwLock::new(HashMap::default())));

    let activity = Some(ActivityData::watching("you sleep"));
    ctx.shard.set_presence(activity, OnlineStatus::Online);
    let connection = lock.get::<DBConnection>().unwrap().clone();
    voice_channels::db::init_next_child_number(&connection)
        .await
        .wrap_err_with(|| eyre!("initializing next child numbers failed!"))?;

    debug!("Finished initializing all guilds!");
    info!("Proceeding to remove all inactive guilds");

    clean_inactive_guilds_from_db(
        &connection,
        &ready
            .guilds
            .iter()
            .map(|guild| guild.id.get() as i64)
            .collect::<Vec<_>>(),
    )
    .await
    .wrap_err_with(|| eyre!("Cleaning inactive guilds from database failed!"))?;
    info!("Finished running ready!");
    Ok(())
}

#[allow(clippy::too_many_lines)]
async fn on_voice_state_update(
    ctx: &SerenityContext,
    old: Option<VoiceState>,
    new: VoiceState,
) -> Result<()> {
    info!("Handling voice state update!");
    info!("New: {new:#?}, Old: {old:#?}");
    let parsed_event = parse_voice_event(ctx, old, new)
        .wrap_err_with(|| eyre!("Parsing voice state event failed!"))?;

    info!("Done parsing voice state event!");

    let (guild_id, joined_channel_id, left_channel_id) = match &parsed_event {
        | ParsedVoiceStateEvent::Joined {
            joined_channel_id,
            guild_id,
            ..
        } => (*guild_id, Some(*joined_channel_id), None),
        | ParsedVoiceStateEvent::Left {
            left_channel_id,
            guild_id,
            ..
        } => (*guild_id, None, Some(*left_channel_id)),
        | ParsedVoiceStateEvent::Changed {
            old_channel_id,
            new_channel_id,
            guild_id,
            ..
        } => (*guild_id, Some(*new_channel_id), Some(*old_channel_id)),
    };

    info!("Parsed event: {:#?}", parsed_event);

    let channels_arr = [left_channel_id, joined_channel_id]
        .map(|v| v.unwrap_or(ChannelId::new(u64::MAX)).get() as i64);

    let channels: &[i64] = match (left_channel_id, joined_channel_id) {
        | (Some(_), Some(_)) => &channels_arr,
        | (Some(_), None) => &channels_arr[..1],
        | (None, Some(_)) => &channels_arr[1..],
        | (None, None) => &[],
    };

    let (parent, children) = voice_channels::db::get_all_children_of_parent(
        &get_db_handle(ctx).await,
        guild_id,
        channels,
    )
    .await
    .wrap_err_with(|| eyre!("Retrieving voice channels failed!"))?
    .ok_or_else(|| eyre!("No parent found!"))?;
    info!("Parent: {:?}, children: {:?}", parent, children);
    if Some(parent.id) == joined_channel_id {
        let parent_channel = ctx.cache.guild_channel(guild_id, parent.id)?;
        let mut map = Map::new();

        map.insert("type".into(), Value::Number(Number::from(2)))
            .drop();
        if let Some(grandparent_id) = parent_channel.parent_id {
            map.insert("parent_id".into(), grandparent_id.get().to_string().into())
                .drop();
        }
        map.insert("name".into(), "Child".into()).drop();
        if let Some(cap) = parent.capacity {
            map.insert("user_limit".into(), Value::Number(Number::from(cap)))
                .drop();
        }
        let mut new = ctx
            .http
            .create_channel(guild_id, &map, Some("Creating new child channel!"))
            .await
            .wrap_err_with(|| {
                eyre!(
                    "Failed at creating new child for channel {}",
                    parent_channel.id.get()
                )
            })?;
        let total_children_number = voice_channels::db::register_child(
            &get_db_handle(ctx).await,
            guild_id,
            parent.id,
            new.id,
        )
        .await
        .wrap_err_with(|| {
            eyre!("Registering child channel in database for server with id {guild_id} failed!")
        })?;
        let map = {
            let guild_channels_map = get_value::<GuildChannels>(&ctx.data).await;
            let lock = guild_channels_map.read().await;
            lock.get(&guild_id).unwrap().clone()
        };

        let mut map_lock = map.write().await;

        map_lock
            .entry(parent.clone())
            .or_default()
            .insert(Child {
                id: new.id,
                number: total_children_number,
                total_children_number,
                template: parent.template.clone(),
            })
            .drop();
        drop(map_lock);
        drop(map);
        voice_channels::updater::update_channel(UpdaterContext {
            template: &parse_template(&parent.template)
                .wrap_err_with(|| eyre!("Parsing template received from database failed!"))?,
            context: SerenityContextWrapper(ctx),
            channel_number: total_children_number,
            total_children_number,
            channel: &mut new,
        })
        .await
        .wrap_err_with(|| eyre!("Updating channel failed!"))?;

        parsed_event
            .member()
            .move_to_voice_channel(&ctx.http, new.id)
            .await
            .wrap_err_with(|| eyre!("Moving member to new channel failed!"))?
            .drop();
    } else if let Some(child) = children.get(&Child {
        id: left_channel_id.unwrap_or(ChannelId::new(u64::MAX)),
        ..Default::default()
    }) {
        let channel = ctx.cache.guild_channel(guild_id, child.id)?;
        let users_connected_number = channel
            .members(&ctx.cache)
            .wrap_err_with(|| eyre!("Could not retrieve channel members from cache!"))?
            .len() as u64;
        if users_connected_number == 0 {
            ctx.http
                .delete_channel(child.id, Some("Deleting empty child channel!"))
                .await
                .wrap_err_with(|| eyre!("Failed to delete channel!"))?
                .drop();
            let db_handle = get_db_handle(ctx).await;
            voice_channels::db::update_next_child_number(&db_handle, parent.id, child.id)
                .await
                .wrap_err_with(|| eyre!("Failed to update next_channel_number!"))?;
            voice_channels::db::delete_child(&db_handle, guild_id, parent.id, child.id)
                .await
                .wrap_err_with(|| eyre!("Failed to delete child from database!"))?;
        }
    }

    for Child {
        id: child_id,
        number: child_number,
        total_children_number,
        template,
    } in children
    {
        debug!("Updating child channel with id {child_id} and number {child_number}",);
        let mut channel = ctx.cache.guild_channel(guild_id, child_id)?;
        voice_channels::updater::update_channel(UpdaterContext {
            template: &parse_template(&template)
                .wrap_err_with(|| eyre!("Parsing template received from database failed!"))?,
            context: SerenityContextWrapper(ctx),
            channel_number: child_number,
            total_children_number,
            channel: &mut channel,
        })
        .await
        .wrap_err_with(|| eyre!("Updating channel failed!"))?;
    }

    Ok(())
}

#[allow(clippy::unused_async)]
async fn on_message_created(_ctx: &SerenityContext, msg: &Message) -> Result<()> {
    trace!("Message created: {}", msg.content);
    Ok(())
}

async fn on_guild_join(ctx: &SerenityContext, guild: &Guild, _is_new: Option<bool>) -> Result<()> {
    info!("Joined guild: {}", guild.name);
    let guild_channels_map = get_value::<GuildChannels>(&ctx.data).await;
    let mut guild_channels_lock = guild_channels_map.write().await;
    let guild_id = guild.id;

    let connection = get_db_handle(ctx).await;

    let all_channels = voice_channels::db::get_all_channels_in_guild(&connection, guild_id)
        .await
        .wrap_err_with(|| eyre!("Retrieving voice channels failed in guild `{guild_id}`!"))?;
    debug!("All channels for guild: {guild_id}: {all_channels:?}");

    let mut deleted_child_ids = Vec::new();
    let mut deleted_parent_ids = Vec::new();

    for (parent, children) in &all_channels {
        debug!("Parent: {parent:#?}, Children: {children:#?}");
        if !guild.channels.contains_key(&parent.id) {
            info!("Parent {} doesn't exist anymore!", parent.id);
            deleted_parent_ids.push(parent.id.get() as i64);
            deleted_child_ids.extend(children.iter().map(|child| child.id.get() as i64));
            continue;
        }
        deleted_child_ids.extend(children.iter().filter_map(|child| {
            guild
                .channels
                .contains_key(&child.id)
                .not()
                .then_some(child.id.get() as i64)
        }));
        debug!("Deleted children: {deleted_child_ids:?}");
    }

    voice_channels::db::remove_dead_channels(
        &get_db_handle(ctx).await,
        &deleted_parent_ids,
        &deleted_child_ids,
    )
    .await
    .wrap_err_with(|| eyre!("Deleting children failed!"))?;
    guild_channels_lock
        .insert(guild_id, Arc::new(RwLock::new(all_channels)))
        .drop();
    debug!("Guild channels after insert: {guild_channels_lock:?}");
    drop(guild_channels_lock);
    info!("Updating voice states for guild: {}", guild_id);
    let mut voice_states = HashMap::default();

    voice_states.extend(guild.voice_states.iter().map(|(k, v)| (*k, v.clone())));
    info!("Finished updating voice states for guild: {}", guild_id);
    debug!("Voice states: {voice_states:?}");

    let voice_states_map = get_value::<VoiceStates>(&ctx.data).await;
    let mut voice_states_lock = voice_states_map.write().await;
    voice_states_lock
        .insert(guild_id, Arc::new(RwLock::new(voice_states)))
        .drop();

    Ok(())
}

async fn on_guild_leave(
    ctx: &SerenityContext,
    incomplete: &UnavailableGuild,
    guild: Option<&Guild>,
) -> Result<()> {
    let guild_id = incomplete.id;
    info!(
        "Left guild: {}",
        guild.as_ref().map_or("Unknown guild", |v| v.name.as_str())
    );

    let db_handle = get_db_handle(ctx).await;

    clean_left_guild_from_db(&db_handle, guild_id)
        .await
        .wrap_err_with(|| eyre!("Cleaning guild with ID `{guild_id}` from database failed!"))?;

    let guild_channels_map = get_value::<GuildChannels>(&ctx.data).await;
    let mut guild_channels_lock = guild_channels_map.write().await;
    guild_channels_lock.remove(&guild_id).drop();

    let voice_states_map = get_value::<VoiceStates>(&ctx.data).await;
    let mut voice_states_lock = voice_states_map.write().await;
    voice_states_lock.remove(&guild_id).drop();

    Ok(())
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum ParsedVoiceStateEvent {
    Joined {
        old:               Option<VoiceState>,
        new:               VoiceState,
        joined_channel_id: ChannelId,
        guild_id:          GuildId,
        member:            Member,
    },
    Left {
        old:             VoiceState,
        new:             VoiceState,
        left_channel_id: ChannelId,
        guild_id:        GuildId,
        member:          Member,
    },
    Changed {
        old:            VoiceState,
        new:            VoiceState,
        old_channel_id: ChannelId,
        new_channel_id: ChannelId,
        guild_id:       GuildId,
        member:         Member,
    },
}

impl ParsedVoiceStateEvent {
    fn member(&self) -> &Member {
        match self {
            | Self::Joined { member, .. }
            | Self::Left { member, .. }
            | Self::Changed { member, .. } => member,
        }
    }
}

fn parse_voice_event(
    _ctx: &SerenityContext,
    old: Option<VoiceState>,
    new: VoiceState,
) -> Result<ParsedVoiceStateEvent> {
    info!("Parsing voice event!");
    let member = new
        .member
        .clone()
        .ok_or_else(|| eyre!("No member provided!"))?;
    let guild_id = new.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?;
    let Some(joined_channel_id) = new.channel_id else {
        let old = old.ok_or_else(|| eyre!("No old voice state provided!"))?;
        let old_channel_id = old
            .channel_id
            .ok_or_else(|| eyre!("No old channel id provided!"))?;
        return Ok(ParsedVoiceStateEvent::Left {
            old,
            new,
            left_channel_id: old_channel_id,
            member,
            guild_id,
        });
    };
    let Some(old) = old else {
        return Ok(ParsedVoiceStateEvent::Joined {
            old,
            new,
            joined_channel_id,
            member,
            guild_id,
        });
    };
    let (old_channel_id, new_channel_id) = match (old.channel_id, new.channel_id) {
        | (None, None) => return Err(eyre!("Both new and old channel id were null")),
        | (Some(old_channel_id), None) =>
            return Ok(ParsedVoiceStateEvent::Left {
                old,
                new,
                left_channel_id: old_channel_id,
                member,
                guild_id,
            }),
        | (None, Some(new_channel_id)) =>
            return Ok(ParsedVoiceStateEvent::Joined {
                old: Some(old),
                new,
                joined_channel_id: new_channel_id,
                member,
                guild_id,
            }),
        | (Some(old_channel_id), Some(new_channel_id)) => (old_channel_id, new_channel_id),
    };

    Ok(ParsedVoiceStateEvent::Changed {
        old,
        new,
        old_channel_id,
        new_channel_id,
        member,
        guild_id,
    })
}

pub(crate) async fn on_event(
    ctx: &SerenityContext,
    event: &FullEvent,
    _framework: FrameworkContext<'_>,
) -> Result<()> {
    let event_span = trace_span!("Discord event");
    async move {
        trace!("Event: {event:#?}");
        #[cfg_attr(feature = "nightly-features", allow(non_exhaustive_omitted_patterns))]
        match event {
            | FullEvent::VoiceStateUpdate { old, new } =>
                on_voice_state_update(ctx, old.clone(), new.clone())
                    .instrument(trace_span!("Voice state update"))
                    .await,
            | FullEvent::Message { new_message } =>
                on_message_created(ctx, new_message)
                    .instrument(trace_span!("Message created"))
                    .await,
            | FullEvent::GuildCreate { guild, is_new } =>
                on_guild_join(ctx, guild, *is_new)
                    .instrument(trace_span!("Guild join"))
                    .await,
            | FullEvent::GuildDelete { incomplete, full } =>
                on_guild_leave(ctx, incomplete, full.as_ref())
                    .instrument(trace_span!("Guild leave"))
                    .await,
            | FullEvent::ChannelUpdate { old, new } =>
                on_channel_update(ctx, old.as_ref(), new)
                    .instrument(trace_span!("Channel update"))
                    .await,
            | FullEvent::ChannelDelete { channel, messages } =>
                on_channel_delete(ctx, channel, messages.as_ref())
                    .instrument(trace_span!("Channel delete"))
                    .await,
            | _ => Ok(()),
        }
    }
    .instrument(event_span)
    .await
}

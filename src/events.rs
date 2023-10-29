use std::env::var;

use eyre::{eyre, Result, WrapErr};
use serenity::model::prelude::*;
use serenity::{async_trait, prelude::*};
use sqlx::postgres::PgPoolOptions;
use tracing::{info, info_span};

use voice_channels::db::TemplatedChannel;

use crate::{
    default_prefix, get_client_id, get_db_handle,
    voice_channels::{self, parser::parse_template, updater::UpdaterContext},
    ClientID, DBConnection, DefaultPrefix,
};

#[non_exhaustive]
pub struct VoiceChannelManagerEventHandler {}

impl VoiceChannelManagerEventHandler {
    pub fn new() -> Self {
        Self {}
    }
}

enum ParsedVoiceStateEvent {
    JoinedChannel {
        old: Option<VoiceState>,
        new: VoiceState,
        joined_channel_id: ChannelId,
        guild_id: GuildId,
        member: Member,
    },
    LeftChannel {
        old: VoiceState,
        new: VoiceState,
        left_channel_id: ChannelId,
        guild_id: GuildId,
        member: Member,
    },
    ChangedChannel {
        old: VoiceState,
        new: VoiceState,
        old_channel_id: ChannelId,
        new_channel_id: ChannelId,
        guild_id: GuildId,
        member: Member,
    },
}

fn parse_voice_event(
    ctx: Context,
    old: Option<VoiceState>,
    new: VoiceState,
) -> Result<Option<ParsedVoiceStateEvent>> {
    let member = new.member.ok_or_else(|| eyre!("No member provided!"))?;
    let guild_id = new.guild_id.ok_or_else(|| eyre!("No guild id provided!"))?;
    let Some(old) = old else {
        return Ok(Some(ParsedVoiceStateEvent::JoinedChannel {
            old,
            new,
            joined_channel_id: new
                .channel_id
                .ok_or_else(|| eyre!("Channel id was unexpectedly missing in new voice state!"))?,
            member,
            guild_id,
        }));
    };
    let (old_channel_id, new_channel_id) = match (old.channel_id, new.channel_id) {
        (None, None) => return Err(eyre!("Both new and old channel id were null")),
        (Some(old_channel_id), None) => {
            return Ok(Some(ParsedVoiceStateEvent::LeftChannel {
                old,
                new,
                left_channel_id: old_channel_id,
                member,
                guild_id,
            }))
        }
        (None, Some(new_channel_id)) => {
            return Ok(Some(ParsedVoiceStateEvent::JoinedChannel {
                old: Some(old),
                new,
                joined_channel_id: new_channel_id,
                member,
                guild_id,
            }))
        }
        (Some(old_channel_id), Some(new_channel_id)) => (old_channel_id, new_channel_id),
    };

    Ok(if old_channel_id == new_channel_id {
        None
    } else {
        Some(ParsedVoiceStateEvent::ChangedChannel {
            old,
            new,
            old_channel_id,
            new_channel_id,
            member,
            guild_id,
        })
    })
}
#[async_trait]
impl EventHandler for VoiceChannelManagerEventHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        let ready_span = info_span!("Ready Span");
        let _guard = ready_span.enter();

        info!("{} is connected!", ready.user.name);
        let mut lock = ctx.data.write().await;
        lock.insert::<DBConnection>(
            PgPoolOptions::new()
                .max_connections(1024)
                .connect(
                    &var("DATABASE_URL")
                        .wrap_err_with(|| {
                            eyre!("Reading database url environment variable failed!")
                        })
                        .unwrap(),
                )
                .await
                .wrap_err_with(|| eyre!("Connecting to database failed!"))
                .unwrap(),
        );
        lock.insert::<ClientID>(get_client_id());
        lock.insert::<DefaultPrefix>(default_prefix().into());
    }

    async fn message(&self, ctx: Context, msg: Message) {
        #[cold]
        async fn mention_handler(
            _handle: &VoiceChannelManagerEventHandler,
            ctx: Context,
            msg: Message,
        ) {
            let prefix =
                crate::prefixes::db::get_prefix(&get_db_handle(&ctx).await, msg.guild_id.unwrap())
                    .await
                    .wrap_err_with(|| eyre!("Retrieving server prefix failed!"))
                    .unwrap();
            let str_ref = match prefix {
                Some(ref prefix) => prefix,
                None => default_prefix(),
            };
            msg.channel_id
                .say(
                    &ctx.http,
                    format!("My prefix for this server is `{}`", str_ref,),
                )
                .await
                .unwrap();
        }
        if msg.mentions_me(ctx.http.clone()).await.unwrap()
            && msg.content == ctx.cache.current_user_id().mention().to_string()
        {
            mention_handler(self, ctx, msg).await;
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let Some(parsed_event) = parse_voice_event(ctx.clone(), old, new)
            .wrap_err_with(|| eyre!("Parsing voice state event failed!"))
            .unwrap() else {
                return;
        };

        let (&guild_id, &channel_id) = match &parsed_event {
            ParsedVoiceStateEvent::JoinedChannel {
                joined_channel_id,
                guild_id,
                ..
            } => (guild_id, joined_channel_id),
            ParsedVoiceStateEvent::LeftChannel {
                left_channel_id,
                guild_id,
                ..
            } => (guild_id, left_channel_id),
            ParsedVoiceStateEvent::ChangedChannel {
                new_channel_id,
                guild_id,
                ..
            } => (guild_id, new_channel_id),
        };

        let channels =
            voice_channels::db::get_all_channels(&get_db_handle(&ctx).await, guild_id, channel_id)
                .await
                .wrap_err_with(|| eyre!("Retrieving voice channels failed!"))
                .unwrap();

        for channel in channels {
            match channel {
                TemplatedChannel::Child {
                    child_id,
                    child_number,
                    total_children_number,
                    template,
                } => {
                    let channel = ctx
                        .cache
                        .guild_channel(child_id)
                        .ok_or_else(|| eyre!("No channel found!"))
                        .unwrap();

                    voice_channels::updater::update_channel(UpdaterContext {
                        template: &parse_template(&template)
                            .wrap_err_with(|| {
                                eyre!("Parsing template received from database failed!")
                            })
                            .unwrap(),
                        channel: &mut channel,
                        context: &ctx,
                        channel_number: child_number,
                        total_children_number,
                        users_connected_capacity: channel.user_limit.ok_or_else(|| {
                            eyre!("No user limit provided for channel with id {channel_id}!")
                        }).unwrap(),
                        users_connected_number: channel
                            .members(&ctx.cache).wrap_err_with(|| eyre!("Could not retrieve channel members from cache!")).unwrap().len() as u64
                    })
                    .await
                    .wrap_err_with(|| eyre!("Updating channel failed!"))
                    .unwrap();
                }
                TemplatedChannel::Parent {
                    parent_id,
                    next_child_number,
                    total_children_number,
                    template,
                } => {
                    let parent_channel = ctx
                        .cache
                        .guild_channel(parent_id)
                        .ok_or_else(|| eyre!("No channel found!"))
                        .unwrap();

                    parent_channel.user_limit

                    voice_channels::updater::update_channel(UpContext {
                        template: &channel.template,
                        channel: &mut channel,
                        context: &ctx,
                        channel_number: next_child_number,
                        total_child_number,
                    })
                    .await
                    .wrap_err_with(|| eyre!("Updating channel failed!"))
                    .unwrap();
                }
            }
        }
    }
}

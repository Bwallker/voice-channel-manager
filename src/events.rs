use std::env::var;

use eyre::{eyre, WrapErr};
use serenity::model::prelude::*;
use serenity::{async_trait, prelude::*};
use sqlx::postgres::PgPoolOptions;
use tracing::{info, info_span};

use voice_channels::db::TemplatedChannel;

use crate::{
    default_prefix, get_client_id, get_db_handle, voice_channels::{self, updater::UpdaterContext}, ClientID, DBConnection,
    DefaultPrefix,
};

#[non_exhaustive]
pub struct VoiceChannelManagerEventHandler {}

impl VoiceChannelManagerEventHandler {
    pub fn new() -> Self {
        Self {}
    }
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
        let channel_id = new
            .channel_id
            .ok_or_else(|| eyre!("No channel id provided!"))
            .unwrap();
        let guild_id = new
            .guild_id
            .ok_or_else(|| eyre!("No guild id provided!"))
            .unwrap();

        let channels =
            voice_channels::db::get_all_channels(&get_db_handle(&ctx).await, guild_id, channel_id)
                .await
                .wrap_err_with(|| eyre!("Retrieving voice channels failed!"))
                .unwrap();

        for channel in channels {
            match channel {
                TemplatedChannel::Child{child_id, child_number, total_children_number, template } => {
                    let channel = ctx.cache.guild_channel(child_id).ok_or_else(|| eyre!("No channel found!")).unwrap();

                    voice_channels::updater::update_channel(UpdaterContext {
                        template: &channel.template,
                        channel: &mut channel,
                        context: &ctx,
                        channel_number,
                        total_child_number,
                    }).await.wrap_err_with(|| eyre!("Updating channel failed!")).unwrap();

                    
                }
                TemplatedChannel::Parent{parent_id, next_child_number, total_children_number } => {
                    let channel = ctx.cache.guild_channel(parent_id).ok_or_else(|| eyre!("No channel found!")).unwrap();

                    voice_channels::updater::update_channel(UpContext {
                        template: &channel.template,
                        channel: &mut channel,
                        context: &ctx,
                        channel_number: next_child_number,
                        total_child_number,
                    }).await.wrap_err_with(|| eyre!("Updating channel failed!")).unwrap();

                    
                }
            }
        }
    }
}

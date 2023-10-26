use std::env::var;

use eyre::{eyre, Context as _};
use serenity::model::prelude::*;
use serenity::{async_trait, prelude::*};
use sqlx::postgres::PgPoolOptions;
use tracing::{info, info_span};

use crate::{default_prefix, get_client_id, ClientID, DBConnection, DefaultPrefix};

pub struct VoiceChannelManagerEventHandler;

impl VoiceChannelManagerEventHandler {
    pub fn new() -> Self {
        Self
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
            let prefix = crate::prefixes::db::get_prefix(
                ctx.data.read().await.get::<DBConnection>().unwrap(),
                msg.guild_id.unwrap(),
            )
            .await
            .wrap_err_with(|| eyre!("Retrieving server prefix failed!"))
            .unwrap();
            let str_ref = match prefix {
                Some(ref prefix)  => prefix,
                None => default_prefix()
            };
            msg.channel_id
                .say(
                    &ctx.http,
                    format!(
                        "My prefix for this server is `{}`",
                        str_ref,
                    ),
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
}

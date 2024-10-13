use eyre::{
    eyre,
    Result,
};
use serenity::{
    all::{
        Cache,
        ChannelId,
        GuildChannel,
        GuildId,
    },
    prelude::{
        TypeMap,
        TypeMapKey,
    },
};
use tokio::sync::RwLock;

pub(crate) async fn get_value<T>(map: &RwLock<TypeMap>) -> T::Value
where
    T: TypeMapKey,
    T::Value: Clone,
{
    let lock = map.read().await;
    lock.get::<T>().unwrap().clone()
}

pub(crate) trait CacheExt {
    fn guild_channel(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<GuildChannel>;
}

impl CacheExt for Cache {
    fn guild_channel(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<GuildChannel> {
        Ok(self
            .guild(guild_id)
            .ok_or_else(|| eyre!("Guild was missing in cache!"))?
            .channels
            .get(&channel_id)
            .ok_or_else(|| eyre!("No channel found!"))?
            .clone())
    }
}

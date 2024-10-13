use std::fmt::{
    self,
    Write,
};

use eyre::{
    eyre,
    Result,
    WrapErr,
};
use serenity::{
    builder::EditChannel,
    client::Context as SerenityContext,
    model::channel::GuildChannel,
};
use tracing::{
    debug,
    info,
};

use super::parser::{
    Template,
    TemplatePart,
};
pub(crate) struct SerenityContextWrapper<'ctx>(pub(crate) &'ctx SerenityContext);

impl fmt::Debug for SerenityContextWrapper<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SerenityContextWrapper")
            .field("data", &"Arc(RwLock(TypeMap{...}))")
            .field("http", &"Arc(Http)")
            .field("cache", &"Arc(Cache)")
            .field("shard", &"Arc(ShardMessenger)")
            .field("shard_id", &self.0.shard_id)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) struct UpdaterContext<'template, 'channel, 'ctx> {
    pub(crate) template:              &'template Template,
    pub(crate) channel:               &'channel mut GuildChannel,
    pub(crate) context:               SerenityContextWrapper<'ctx>,
    pub(crate) channel_number:        u64,
    pub(crate) total_children_number: u64,
}

pub(crate) async fn update_channel(ctx: UpdaterContext<'_, '_, '_>) -> Result<()> {
    debug!("UpdaterContext: {ctx:#?}");
    let mut new_name = String::new();

    for part in &ctx.template.parts {
        debug!("part: {:?}", part,);
        match part {
            | TemplatePart::String(s) => new_name.push_str(s),
            | TemplatePart::ChannelNumber => write!(new_name, "{}", ctx.channel_number)
                .map_err(|e| eyre!(e))
                .wrap_err_with(|| eyre!("Writing channel number into string failed!"))?,
            | TemplatePart::ChildrenInTotal => write!(new_name, "{}", ctx.total_children_number)
                .map_err(|e| eyre!(e))
                .wrap_err_with(|| eyre!("Writing total child count into string failed!"))?,
        }
    }

    debug!("new_name: {}", new_name,);
    if new_name != ctx.channel.name {
        let context = ctx.context.0.clone();
        debug!("Cloned context.");
        ctx.channel
            .edit(context, EditChannel::new().name(new_name))
            .await
            .map_err(|e| eyre!(e))
            .wrap_err_with(|| eyre!("Failed to rename channel!"))?;
    }
    info!("Successfully renamed channel!");

    Ok(())
}

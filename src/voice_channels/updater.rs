use super::parser::{Template, TemplatePart};
use eyre::{eyre, Result, WrapErr};
use serenity::{client::Context, model::channel::GuildChannel};
use std::fmt::Write;

pub struct UpdaterContext<'template_content, 'template, 'channel, 'ctx> {
    pub template: &'template Template<'template_content>,
    pub channel: &'channel mut GuildChannel,
    pub context: &'ctx Context,
    pub channel_number: u64,
    pub total_children_number: u64,
}

pub async fn update_channel(ctx: UpdaterContext<'_, '_, '_, '_>) -> Result<()> {
    let mut new_name = String::new();

    for part in &ctx.template.parts {
        match part {
            TemplatePart::String(s) => new_name.push_str(s),
            TemplatePart::ChannelNumber => write!(new_name, "{}", ctx.channel_number)
                .map_err(|e| eyre!(e))
                .wrap_err_with(|| eyre!("Writing channel number into string failed!"))?,
            TemplatePart::ChildrenInTotal => write!(new_name, "{}", ctx.total_children_number)
                .map_err(|e| eyre!(e))
                .wrap_err_with(|| eyre!("Writing total child count into string failed!"))?,
        }
    }

    ctx.channel
        .edit(ctx.context.clone(), |c| c.name(new_name))
        .await
        .map_err(|e| eyre!(e))
        .wrap_err_with(|| eyre!("Failed to rename channel!"))
}

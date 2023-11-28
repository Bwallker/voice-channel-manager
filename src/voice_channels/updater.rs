use super::parser::{Template, TemplatePart};
use eyre::{eyre, Result, WrapErr};
use serenity::{client::Context, model::channel::GuildChannel};
use std::fmt::Write;

pub(crate) struct UpdaterContext<'template, 'channel, 'ctx> {
    pub(crate) template: &'template Template,
    pub(crate) channel: &'channel mut GuildChannel,
    pub(crate) context: &'ctx Context,
    pub(crate) channel_number: u64,
    pub(crate) total_children_number: u64,
    pub(crate) users_connected_number: u64,
    pub(crate) users_connected_capacity: u64,
}

pub(crate) async fn update_channel(ctx: UpdaterContext<'_, '_, '_>) -> Result<()> {
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
            TemplatePart::ConnectedUsersNumber => {
                write!(new_name, "{}", ctx.users_connected_number)
                    .map_err(|e| eyre!(e))
                    .wrap_err_with(|| eyre!("Writing connected users count into string failed!"))?;
            }

            TemplatePart::ConnectedUserCapacity => {
                write!(new_name, "{}", ctx.users_connected_capacity)
                    .map_err(|e| eyre!(e))
                    .wrap_err_with(|| {
                        eyre!("Writing connected users capacity into string failed!")
                    })?;
            }
        }
    }

    ctx.channel
        .edit(ctx.context.clone(), |c| c.name(new_name))
        .await
        .map_err(|e| eyre!(e))
        .wrap_err_with(|| eyre!("Failed to rename channel!"))
}

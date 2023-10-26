use eyre::Result;
use serenity::model::prelude::*;
use serenity::prelude::*;
use tracing::info;

use crate::get_client_id;

pub struct Command<'a> {
    command: &'a str,
    args: Vec<&'a str>,
}

impl Command<'_> {
    pub fn get_nth_arg(&self, n: usize) -> Option<&str> {
        self.args.get(n).copied()
    }

    pub fn args(&self) -> &[&str] {
        &self.args
    }

    pub fn args_iter(&self) -> impl Iterator<Item = &str> {
        self.args.iter().copied()
    }
}

pub fn parse_command<'a>(context: &Context, command: &'a Message) -> Result<Command<'a>> {
    let mut args = command.content.split_whitespace();
    info!("Command: {}", command.content);
    if command
        .content
        .trim()
        .starts_with(&format!("<@{}>", get_client_id()))
    {
        info!("Command is a mention!");
        args.next();
    }
    args.next();
    Ok(Command {
        command: &command.content,
        args: args.collect(),
    })
}

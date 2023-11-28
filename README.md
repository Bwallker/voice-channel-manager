# Voice Channel Manager

## Description

This is a Discord bot that allows users to automatically create and manage voice channels. It is written in Rust using the [Serenity](https://github.com/serenity-rs/serenity) library. It is currently in development.

## Usage

The bot is currently not hosted anywhere, so you will have to host it yourself. To do so, you will need to have [Rust](https://www.rust-lang.org/tools/install) and [Postgres](https://www.postgresql.org/download) installed. You will also need to create a Discord bot and invite it to your server. You can find instructions on how to do that [here](https://discordpy.readthedocs.io/en/latest/discord.html). Read the installation guide [here](#installation) for more details.

## Purpose

The purpose of this bot is to allow users to create, manage and delete voice channels automatically as user demand ebbs and flows. This is useful for servers that have a large number of users that are not always active at the same time. It is also useful for servers that have a large number of users that are active at the same time, but are not always active in the same channels.

### Functionality

The bot automatically creates and deletes channels using the concept of parent and child channels. A parent channel is a channel that will spawn a new child anytime someone joins the parent channel and move the user to the child channel. A child channel is a channel that will be deleted when it is empty. Parent channels creates child channels using a template in order to set the names of the children.

#### Templates

 Parent channels use templates to set the names of their child channels. Templates are strings of text that contain special directives in curly braces that are replaced with values when the child channel is created. Curly braces may be escaped by using two curly braces in a row. The following directives are available:

##### Directives

1. `{#}`: The number of the child channel. This is incremented for each child channel created, and decremented as children are deleted. It is guaranteed that two living children will never have the same number.
2. `{%}`: The total number of sibling channels currently living (count also includes self).

###### Example templates

`Gaming channel number: {#}`
`Gaming channel number: {#}/{%}`
`Curly braces: {{}}`

### Commands

All commands are invoked by sending a message in your server prefixed with a prefix followed by a command name and a whitespace delimited set of arguments. The prefix is either the `DEFAULT_PREFIX` or your servers configured prefix. `DEFAULT_PREFIX` will always work in addition to whatever server specific prefixes have been configured.The following commands are available:

#### Channel commands

##### `vc/create_channel`

Creates a new voice channel with the given name. The channel will be created in the root category of your server. Requires two arguments, the name of the channel and the template to use when creating child channels.

##### `vc/alter_template`

Changes the template used by the given channel. Requires two arguments, the ID of the channel and the new template. Does NOT require the channel to be a parent channel to work.

###### Aliases
`vc/alter_channel`, `vc/alter_parent`

##### `vc/change_capacity`

Changes the capacity of child channels created by the given parent channel. Requires two arguments, the ID of the channel and the new capacity.

###### Aliases

`vc/change_cap`, `vc/set_cap`, `vc/set_capacity`

#### Prefix commands

##### `vc/change_prefix`

Changes the prefix used to invoke the bot in your server. Requires one argument, the new prefix.

###### Aliases
`vc/set_prefix`

##### `vc/reset_prefix`

Resets the prefix used to invoke the bot in your server to the default prefix.

### Configuration

#### Environment Variables

The bot is configured using environment variables.

##### Required Variables

1. `DISCORD_TOKEN`: The token of your Discord bot.
2. `DISCORD_CLIENT_ID`: The client ID of your Discord bot.
3. `DATABASE_URL`: The URL of your PostgreSQL database.
4. `DATABASE_USER`: The username used in your PostgreSQL database.
5. `DATABASE_PASSWORD`: The password of your PostgreSQL database. This variable is optional if you do not have a password set.
6. `DATABASE_ROLE`: The role used in of your PostgreSQL database.



##### Optional Variables

1. `RUST_LOG`: The log level of the bot. See details [here](https://docs.rs/env_logger/latest/env_logger/#enabling-logging). Defaults to `voice_channel_manager=debug,info`.
2. `DISCORD_PREFIX`: The prefix used to invoke the bot. Defaults to `vc/`.


## Installation

1. Clone the repository.
2. Create a .env file in the root of the repository and fill it with the required environment variables. See the [Configuration](#configuration) section for more details. A .env file is not required if you set the environment variables in some other way.
3. Create and migrate the database by running `sqlx database setup` in the root of the repository.
4. Run the bot by running `cargo run` or `cargo run --release` in the root of the repository. You can also use the included dockerfile to run the bot in a docker container. Please note that the database must be run separately if you use the dockerfile.
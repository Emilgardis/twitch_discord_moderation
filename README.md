# Twitch Discord Moderation | Log mod actions with discord

Log moderator actions with a discord channel.

Example usage with docker-compose

```yml
version: "3"

services:
  twitch-discord-moderation:
    image: emilgardis/twitch-discord-moderation:latest
    env_file: .env
    environment:
      RUST_LOG: "info"
    restart: "unless-stopped"
    volumes:
      - ./.dcf_secret:/app/.dcf_secret # only needed if you want to use DCF
```

create an empty file called `.dcf_secret` (and make sure it's only readable by trusted users)

and then create a `.env` file containing the following (make sure to replace `CHANNEL_LOGIN` or omit it to use the owner of the token):

```txt
DCF_OAUTH_CLIENT_ID=ytf4qimvfnkm2egtyxi4ckm4bex49e # This is a client id created for this application. Feel free to use it.
CHANNEL_LOGIN=justintv
DISCORD_WEBHOOK=https://discordapp.com/api/webhooks/111111111111/aaaaaaaaaaaaaaa
RUST_LOG=info
```

With the above config, the bot will post a message to the webhook prompting a user to login with a link. Once authorized, the bot will monitor moderation actions on the specified channel `justintv` (if the user that authorized has permission to do that) and post them to the discord webhook.

## Features

This application also supports getting an oauth2 token from an external service on url. This service should return a token in a json body where the token string is in the field `access_token`, if not, specify the path with a pointer.

## Commandline options

<!--BEGIN commandline options-->
```text
Bot to send twitch moderator actions to a discord webhook

Usage: twitch-discord-moderation [OPTIONS] --discord-webhook <DISCORD_WEBHOOK>

Options:
      --discord-webhook <DISCORD_WEBHOOK>
          URL to discord webhook

      --access-token <ACCESS_TOKEN>
          OAuth2 Access token

      --channel-login <CHANNEL_LOGIN>
          Name of channel to monitor. If left out, defaults to owner of access token

      --channel-id <CHANNEL_ID>
          User ID of channel to monitor. If left out, defaults to owner of access token

      --oauth2-service-url <OAUTH2_SERVICE_URL>
          URL to service that provides OAuth2 token. Called on start and whenever the token needs to be refreshed.

          This application does not do any refreshing of tokens.

      --oauth2-service-key <OAUTH2_SERVICE_KEY>
          Bearer key for authorizing on the OAuth2 service url

      --oauth2-service-pointer <OAUTH2_SERVICE_POINTER>
          Grab token by pointer. See https://tools.ietf.org/html/rfc6901

      --oauth2-service-refresh <OAUTH2_SERVICE_REFRESH>
          Grab a new token from the OAuth2 service this many seconds before it actually expires. Default is 30 seconds

      --dcf-oauth-client-id <DCF_OAUTH_CLIENT_ID>
          Client id to get a token. Stores the token data in the path specified by `--dcf-secret` (client id and optional secret is not stored)

      --dcf-oauth-client-secret <DCF_OAUTH_CLIENT_SECRET>
          Client secret to get a token. Only needed for confidential applications

      --dcf-secret-path <DCF_SECRET_PATH>
          Path for storing DCF oauth

          [default: ./.dcf_secret]

      --channel-bot-name <CHANNEL_BOT_NAME>
          Name of channel bot

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

```
<!--END commandline options-->

<h5> License </h5>

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>

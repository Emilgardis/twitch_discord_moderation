Twitch Discord Moderation | Sync mod actions with discord
============================================

Sync moderator actions with a discord channel.

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
```

and the `.env`

```txt
ACCESS_TOKEN=0123456789abcdefghijABCDEFGHIJ
CHANNEL_LOGIN=justintv
DISCORD_WEBHOOK=https://discordapp.com/api/webhooks/111111111111/aaaaaaaaaaaaaaa
RUST_LOG=info
```

This application also supports getting an oauth2 token from an external service on url. This service should return a token in a json body where the token string is in the field `access_token`, if not, specify the path with a pointer.

<!--BEGIN commandline options-->
```
twitch-discord-moderation 0.4.0
Bot to send twitch moderator actions to a discord webhook

USAGE:
    twitch-discord-moderation [OPTIONS] --discord-webhook <DISCORD_WEBHOOK>

OPTIONS:
        --access-token <ACCESS_TOKEN>
            OAuth2 Access token

            [env: ACCESS_TOKEN]

        --channel-bot-name <CHANNEL_BOT_NAME>
            Name of channel bot

            [env: CHANNEL_BOT_NAME]

        --channel-id <CHANNEL_ID>
            User ID of channel to monitor. If left out, defaults to owner of access token

            [env: CHANNEL_ID]

        --channel-login <CHANNEL_LOGIN>
            Name of channel to monitor. If left out, defaults to owner of access token

            [env: CHANNEL_LOGIN]

        --discord-webhook <DISCORD_WEBHOOK>
            URL to discord webhook

            [env: DISCORD_WEBHOOK]

    -h, --help
            Print help information

        --oauth2-service-key <OAUTH2_SERVICE_KEY>
            Bearer key for authorizing on the OAuth2 service url

            [env: OAUTH2_SERVICE_KEY]

        --oauth2-service-pointer <OAUTH2_SERVICE_POINTER>
            Grab token by pointer. See https://tools.ietf.org/html/rfc6901

            [env: OAUTH2_SERVICE_POINTER]

        --oauth2-service-refresh <OAUTH2_SERVICE_REFRESH>
            Grab a new token from the OAuth2 service this many seconds before it actually expires.
            Default is 30 seconds

            [env: OAUTH2_SERVICE_REFRESH]

        --oauth2-service-url <OAUTH2_SERVICE_URL>
            URL to service that provides OAuth2 token. Called on start and whenever the token needs
            to be refreshed.

            This application does not do any refreshing of tokens.

            [env: OAUTH2_SERVICE_URL]

    -V, --version
            Print version information

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
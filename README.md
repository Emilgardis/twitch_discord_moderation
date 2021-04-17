Twitch Discord Moderation | Sync mod actions with discord
============================================

You'll need to install rust to run this bot. Or use docker

Install with [rustup.rs](https://rustup.rs/)

```bash
# clone the repository
$ git clone https://github.com/Emilgardis/twitch_discord_moderation.git
# cd into it
cd twitch_discord_moderation
# set .env file to untracked
git update-index --assume-unchanged .env
# edit .env file or set ENV vars accordingly
$ cat .env # do this with your favorite editor, or set env vars
DISCORD_WEBHOOK=<path to discord webhook>
ACCESS_TOKEN=<oauth token, need to have channel:moderate>
CHANNEL_LOGIN=<login name of channel to watch, use the channel owners token for more pubsub messages>
CHANNEL_BOT_NAME=<bot used in channel, optional>
# compile and run
$ cargo run --release
```
# or use docker compose
$ docker-compose up
```

This application also supports getting an oauth2 token from an external service on url `OAUTH2_SERVICE_URL`. This service should return a token in a json body where the token string is in the field `access_token` or `token`, if not, specify the path with `OAUTH2_SERVICE_JQ`.

```
OPTIONS:
        --access-token <access-token>
            OAuth2 Access token [env: ACCESS_TOKEN]

        --channel-bot-name <channel-bot-name>
            Name of channel bot [env: CHANNEL_BOT_NAME]

        --channel-id <channel-id>
            User ID of channel to monitor. If left out, defaults to owner of access token [env:
            CHANNEL_ID]

        --channel-login <channel-login>
            Name of channel to monitor. If left out, defaults to owner of access token [env:
            CHANNEL_LOGIN]

        --discord-webhook <discord-webhook>
            URL to discord webhook [env: DISCORD_WEBHOOK]

        --oauth2-service-key <oauth2-service-key>
            Bearer key for authorizing on the OAuth2 service url [env: OAUTH2_SERVICE_KEY]

        --oauth2-service-refresh <oauth2-service-refresh>
            Grab a new token from the OAuth2 service this many seconds before it actually expires.
            Default is 30 seconds [env: OAUTH2_SERVICE_REFRESH]

        --oauth2-service-url <oauth2-service-url>
            URL to service that provides OAuth2 token. Called on start and whenever the token needs
            to be refreshed.

            This application does not do any refreshing of tokens. [env: OAUTH2_SERVICE_URL]
```


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
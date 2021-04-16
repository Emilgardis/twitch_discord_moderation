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
RUST_LOG=info
DISCORD_WEBHOOK="<path to discord webhook>"
BROADCASTER_OAUTH2="<broadcaster oauth token, need to have channel:moderate>"
CHANNEL_BOT_NAME="<bot used in channel, optional>"
# compile and run
$ cargo run --release
```
# or use docker compose
$ docker-compose up
```

This application also supports getting an oauth2 token from an external service on url `OAUTH2_SERVICE_URL`. This service should return a token in a json body where the token string is in the field `access_token` or `token`, if not, specify the path with 

```
$ cat .env
RUST_LOG=info
DISCORD_WEBHOOK="<path to discord webhook>"
OAUTH2_SERVICE_URL="<path to the service, include query parameters if needed to get the correct token>"
OAUTH2_SERVICE_KEY="<your secure bearer token to authenticate on the service>"
CHANNEL_BOT_NAME="<bot used in channel, optional>"
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
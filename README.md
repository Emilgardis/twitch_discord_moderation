Twitch Discord Moderation | Sync mod actions with discord
============================================

You'll need to install rust to run this bot.

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
BROADCASTER_OAUTH="<broadcaster oauth token, need to have channel:moderate>"
CHANNEL_BOT_NAME="<bot used in channel, optional>"
# compile and run
$ cargo run --release
# or use docker compose
$ docker-compose up
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
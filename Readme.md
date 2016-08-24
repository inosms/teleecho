# teleecho

[![Build Status](https://travis-ci.org/inosms/teleecho.svg?branch=master)](https://travis-ci.org/inosms/teleecho)

a small command to redirect output via a Telegram bot to your Telegram account.

![](https://dl.dropboxusercontent.com/u/36945131/out.gif)


## Installation

1. Install Rust
2. clone this repository
3. ```cd teleecho``` 
4. ```cargo install teleecho```

## Setup

In order to be able to forward messages you must first create a telegram bot and then create a connection between this bot and your telegram account.
Each of this _bot -> account_ sets are called a _connection_.

You can have multiple of those connections. This is handy if you for example log the output of your backup script and the output of another command, but dont want both to dump their logs into the same chat. Thus you can create two bots and use a unique bot for each task.


1. Talk to [botfather](https://telegram.me/botfather) to obtain a token for a new bot, which will be used to forward your messages.
2. ``` teleecho new <TOKEN> <NAME FOR THIS CONNECTION>```
3. Go to your Telegram app, initiate the conversation with the bot and send the displayed number.

## Usage

Once you have setup a connection
If you only have one connection registered, then this will suffice

```
fancy-command | teleecho
```

If you have more than one connection (e.g. different bots or different endpoints) then you have to specify the connection name
```
fancy-command | teleecho backupbot
```

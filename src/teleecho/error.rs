extern crate telegram_bot;
extern crate serde_json;

error_chain! {
    foreign_links{
        ::std::io::Error, Io;
        self::serde_json::Error, SerdeJson;
        self::telegram_bot::Error, TelegramBot;
        ::std::str::Utf8Error, Utf8Error;
    }

    errors {
        ConfigConnectionNotExist {
            description("specified connection does not exist")
            display("specified connection does not exist")
        }
    }
}
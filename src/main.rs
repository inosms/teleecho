extern crate clap;
extern crate rand;

use clap::{Arg, App, SubCommand, AppSettings};
mod teleecho;
use teleecho::teleecho::TeleechoProcessor;
use teleecho::config::Config;
use teleecho::error::Error;
use std::fs::OpenOptions;

macro_rules! unwrap_or_return {
    ($expr:expr) => (
        match $expr {
            Ok(r) => r,
            Err(e) => {
                println!("error: {}",e);
                return
            }
        }
    );
    ($expr:expr,$message:expr) => (
        match $expr {
            Ok(r) => r,
            Err(e) => {
                println!("error {}: {}",$message,e);
                return
            }
        }
    )
}


fn process_input(telelog_bot: &mut TeleechoProcessor) {
    use std::io;
    use std::io::Read;

    let mut input = [0; 400];

    loop {
        match io::stdin().read(&mut input) {
            Ok(n) => {
                match std::str::from_utf8(&input[..n]) {
                    Ok(r) => {
                        telelog_bot.append_to_input_buffer(&r);

                        // if nothing is read anymore; it has ended
                        if n == 0 {
                            break;
                        }
                    }
                    Err(err) => println!("Error while decoding into string [{}]", err),
                }
            }
            Err(error) => println!("error: {}", error),
        }
    }
}

// create the clap app and return the matches
fn create_clap_app<'a, 'b>() -> clap::ArgMatches<'a>
    where 'a: 'b
{
    App::new("teleecho")
        .setting(AppSettings::VersionlessSubcommands)
        .setting(AppSettings::ColoredHelp)
        .version("0.0.1")
        .about("forwards input via telegram to user")
        .arg(Arg::with_name("connection")
                 .value_name("CONNECTION NAME")
                 .help("name of the connection to use for sending")
                 .required(false)
                 .takes_value(true))
        .arg(Arg::with_name("config")
                 .short("c")
                 .long("config")
                 .value_name("FILE")
                 .help("path to config file; defaults to ~/.teleecho.conf")
                 .required(false)
                 .takes_value(true))
        .subcommand(SubCommand::with_name("new")
                        .about("registers bot to user connection")
                        .setting(AppSettings::ColoredHelp)
                        .arg(Arg::with_name("token")
                                 .help("token from botfather to send from")
                                 .required(true))
                        .arg(Arg::with_name("name")
                                 .takes_value(true)
                                 .help("name to specify this connection")
                                 .required(true)))
        .subcommand(SubCommand::with_name("list")
                        .about("list all connections")
                        .setting(AppSettings::ColoredHelp))
        .subcommand(SubCommand::with_name("remove")
                        .about("removes a connection")
                        .arg(Arg::with_name("name")
                                 .help("print debug information verbosely")
                                 .takes_value(true)
                                 .required(true))
                        .setting(AppSettings::ColoredHelp))
        .get_matches()
}


fn main() {
    let matches = create_clap_app();

    // at first get the name of the config file, or if none specified the default path
    let config_file: std::path::PathBuf = match matches.value_of("config") {
        Some(t) => std::path::PathBuf::from(t),
        None => {
            let mut path = unwrap_or_return!(std::env::home_dir().ok_or(Error::HomePathNotFound),
                                             "while getting home directory");
            path.push(".teleecho.conf");
            path
        }
    };

    // then get the name of the connection (None is not specified)
    let connection = matches.value_of("connection");

    // now try to open/create the config file
    let mut f = unwrap_or_return!(OpenOptions::new()
                                      .read(true)
                                      .write(true)
                                      .create(true)
                                      .append(false)
                                      .open(&config_file),
                                  "while opening config file");

    // if successfully opened, try to parse the config file to a config object
    let mut config = unwrap_or_return!(Config::parse(&mut f), "while parsing config file");

    // handle the new subcommand
    if let Some(matches) = matches.subcommand_matches("new") {
        // is required, thus must be Some(...)
        let token = matches.value_of("token").unwrap();
        let name = matches.value_of("name").unwrap();

        // do not allow whitespace in connection name
        let name_without_whitespace = name.split_whitespace().collect::<Vec<&str>>().join("-");

        match config.get(Some(&name_without_whitespace)) {
            Ok(_) => {
                println!("name already taken!");
                return;
            }
            Err(_) => {}
        }

        match teleecho::teleecho::register_connection(token) {
            Some((token, id)) => {
                unwrap_or_return!(config.add_entry(name_without_whitespace.clone(), token, id),
                                  "while adding entry");

                unwrap_or_return!(config.save_to(&mut f), "while saving configuration");

                println!("new connection successfully created: {}",
                         name_without_whitespace);
            }
            None => {}
        }
    }
    // handle the list subcommand
    else if let Some(_) = matches.subcommand_matches("list") {
        config.list();
    }
    // handle the remove subcommand
    else if let Some(matches) = matches.subcommand_matches("remove") {
        let to_remove = matches.value_of("name").unwrap();

        unwrap_or_return!(config.remove(&to_remove), "while removing connection");
        unwrap_or_return!(config.save_to(&mut f), "while saving configuration");
    }
    // if no subcommand was specified, start sending
    else {
        let (token, user) = unwrap_or_return!(config.get(connection),
                                              "while retrieving connection");

        match TeleechoProcessor::create(&token, user) {
            Ok(mut tp) => process_input(&mut tp),
            Err(e) => println!("Error while creating bot instance {}", e),
        }
    }
}

extern crate clap;
extern crate rand;

use clap::{Arg, App, SubCommand, AppSettings};
mod teleecho;
use teleecho::teleecho::TeleechoProcessor;
use teleecho::config::Config;

use std::fs::OpenOptions;

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


fn main() {
    let matches = App::new("teleecho")
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
                      .get_matches();

    let config_file: std::path::PathBuf = match matches.value_of("config") {
        Some(t) => std::path::PathBuf::from(t),
        None => {
            match std::env::home_dir() {
                Some(mut path) => {
                    path.push(".teleecho.conf");
                    path
                }
                None => {
                    println!("unable to get home directory!");
                    return;
                }
            }
        }
    };

    let connection = matches.value_of("connection");

    let mut f = match OpenOptions::new()
                          .read(true)
                          .write(true)
                          .create(true)
                          .append(false)
                          .open(&config_file) {
        Ok(f) => f,
        Err(e) => {
            println!("unable to access file {}: {}", config_file.display(), e);
            return;
        }
    };

    let mut config = match Config::parse(&mut f) {
        Ok(c) => c,
        Err(e) => {
            println!("could not parse config file: {}", e);
            return;
        }
    };

    if let Some(matches) = matches.subcommand_matches("new") {
        // is required, thus must be Some(...)
        let token = matches.value_of("token").unwrap();
        let name = matches.value_of("name").unwrap();
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
                match config.add_entry(name_without_whitespace.clone(), token, id) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("error while adding entry: {}", e);
                        return;
                    }

                }
                match config.save_to(&mut f) {
                    Ok(_) => {
                        println!("new connection successfully created: {}",
                                 name_without_whitespace);
                        return;
                    }
                    Err(e) => {
                        println!("error while saving configuration: {}", e);
                        return;
                    }
                }
            }
            None => return,
        }
    } else if let Some(_) = matches.subcommand_matches("list") {
        config.list();
        return;
    } else if let Some(matches) = matches.subcommand_matches("remove") {
        let to_remove = matches.value_of("name").unwrap();

        match config.remove(&to_remove) {
            Ok(_) => {
                match config.save_to(&mut f) {
                    Ok(_) => {}
                    Err(e) => println!("error while saving configuration: {}", e),
                }
            }
            Err(e) => println!("error while removing connection: {}", e),
        }
        return;
    }


    let (token, user) = match config.get(connection) {
        Ok((t, u)) => (t, u),
        Err(e) => {
            println!("error while retrieving connection: {}", e);
            return;
        }
    };

    match TeleechoProcessor::create(&token, user) {
        Ok(mut tp) => process_input(&mut tp),
        Err(e) => println!("Error while creating bot instance {}", e),
    }
}

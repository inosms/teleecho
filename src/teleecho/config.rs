use std::io::prelude::*;
use std::fs::File;
extern crate serde_json;

use teleecho::error::*;

pub struct Config {
    /// the entries are name, bot token, user id
    entries: Vec<(String, String, i64)>,
}

impl Config {
    /// given a file this reads the content and tries to parse it into a Config object
    pub fn parse(file: &mut File) -> Result<Config> {

        let mut content = String::new();

        try!(file.read_to_string(&mut content));

        // if file was created, there is nothing to read, so create an empty config object
        if content.len() == 0 {
            Ok(Config { entries: vec![] })
        }
        // otherwise try to parse the file content into a configuration
        else {
            Ok(Config { entries: try!(serde_json::from_str(&content)) })
        }
    }

    /// converts the config object into a string, that can be written to a file
    fn to_string(&self) -> Result<String> {
        Ok(try!(serde_json::to_string(&self.entries)))
    }

    /// given a name, bot token and user id this tries to store this in the internal
    /// list. 
    /// this may fail if the same name already exists
    pub fn add_entry(&mut self, name: String, token: String, user_id: i64) -> Result<()> {
        for &(ref n, _, _) in &self.entries {
            if n == &name {
                return Err("config entry already exists".into());
            }
        }

        self.entries.push((name, token, user_id));
        Ok(())
    }

    /// given a file this 
    pub fn save_to(&self, file: &mut File) -> Result<()> {

        // get to the first position of the file to override everything
        try!(file.seek(::std::io::SeekFrom::Start(0)));

        let to_write = try!(self.to_string());
        let to_write_bytes = to_write.as_bytes();

        // write the config to file
        try!(file.write_all(to_write_bytes));

        // then trim the file to the size of the written.
        // this is neccessary as when new config file size is
        // smaller than before, the rest would remain and mess up everything
        try!(file.set_len(to_write_bytes.len() as u64));

        Ok(())
    }

    /// given a connection this returns the token and id for the given
    /// connection, Error if non existent
    ///
    /// given no connection this returns the token and id if there is only one
    /// connection registered, Error otherwise
    pub fn get(&self, connection: Option<&str>) -> Result<(String, i64)> {
        match connection {
            Some(con) => {
                for &(ref n, ref t, ref i) in &self.entries {
                    if n == con {
                        return Ok((t.clone(), i.clone()));
                    }
                }
                Err(ErrorKind::ConfigConnectionNotExist.into())
            }
            None => {
                if self.entries.len() == 1 {
                    let (_, ref t, ref i) = self.entries[0];
                    Ok((t.clone(), i.clone()))
                } else {
                    Err(format!("as no connection was given, the default would be used, but \
                                 there does not exist one, but {} connections to choose from",
                                self.entries.len())
                            .into())
                }
            }
        }
    }

    /// prints out a list of all contained connections on the command line
    pub fn list(&self) {
        for &(ref n, _, _) in &self.entries {
            println!("{}", n);
        }
    }

    /// tries to remove the given connection; 
    /// this may fail if the given connection is not in the list
    pub fn remove(&mut self, to_remove: &str) -> Result<()> {

        // get the index of the one to remove
        let mut to_remove_index = None;
        let mut current_index = 0;
        for &(ref n, _, _) in &self.entries {
            if n == to_remove {
                to_remove_index = Some(current_index);
                break;
            }
            current_index += 1;
        }

        // if was found, remove, otherwise return error
        if to_remove_index.is_some() {
            self.entries.remove(to_remove_index.unwrap());
            Ok(())
        } else {
            Err(ErrorKind::ConfigConnectionNotExist.into())
        }
    }
}
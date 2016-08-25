extern crate telegram_bot;
extern crate time;
extern crate rand;

use rand::Rng;
use teleecho::error::*;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::collections::vec_deque::VecDeque;

#[derive(Debug)]
enum MessageBuffer {
    /// if the given text was preceded by a carriage return
    CarriageReturn(String),

    /// if the given text was not preceded by a carriage return
    Newline(String),
}

/// These are sent from the TeleechoProcessor to the sender to signal
/// if a new element was added to the queue or the processor has ended
#[derive(Debug)]
enum BufferChangeEvent {
    NewElement,
    Kill,
}

struct TeleechoSender {
    /// the last sent message object,
    /// this is needed to be able to edit the last message
    last_sent_message: Option<telegram_bot::Message>,

    /// reference to the api
    api: telegram_bot::Api,

    /// a buffer that stores the messages to be sent
    message_buffer: Arc<Mutex<VecDeque<MessageBuffer>>>,

    /// time in ns when the last message was sent
    last_send_time: u64,

    /// the id to send the messages to
    user_id: i64,
}

impl TeleechoSender {
    fn create(api: telegram_bot::Api,
              user_id: i64)
              -> (Sender<BufferChangeEvent>,
                  JoinHandle<()>,
                  Arc<Mutex<VecDeque<MessageBuffer>>>) {

        // create the sender object
        let ts = TeleechoSender {
            last_sent_message: None,
            api: api,
            message_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(4096))),
            last_send_time: 0,
            user_id: user_id,
        };

        // create the copy of the buffer, where to processor writes to
        let buffer_copy = ts.message_buffer.clone();

        // and the sender/receiver object for communication
        let (sender, receiver) = mpsc::channel();

        // now spawn the thread
        let handle = thread::spawn(move || TeleechoSender::send_loop(ts, receiver));

        // return the necessary parts
        (sender, handle, buffer_copy)
    }


    fn send_loop(mut ts: TeleechoSender, receiver: Receiver<BufferChangeEvent>) {
        loop {
            // the loop receives an event for every new message that is appended
            // or the kill request
            let event = receiver.recv().unwrap();

            // find out which was sent
            match event {
                BufferChangeEvent::Kill => return,
                BufferChangeEvent::NewElement => {

                    let time_diff = time::precise_time_ns() - ts.last_send_time;

                    // send only every second
                    if time_diff <= 1000000000u64 && ts.last_send_time != 0 {
                        thread::sleep(::std::time::Duration::new(0,(1000000000u64 -
                                                                          time_diff) as u32));
                    }

                    // if a new message event is received this does not mean, that
                    // the buffer still has a message, as with the last message event this
                    // message could also have been sent already, as the messages get combined
                    if ts.message_buffer.lock().unwrap().len() > 0 {

                        let to_send = TeleechoSender::combine_messages(&mut ts.message_buffer);

                        match to_send {
                            MessageBuffer::Newline(msg) => ts.send(msg),
                            MessageBuffer::CarriageReturn(msg) => ts.override_last(msg),
                        }

                        // telegram seems to store the end of the request as time
                        // if timed before sending one gets a lot of timeouts
                        ts.last_send_time = time::precise_time_ns();
                    }
                }
            }
        }
    }

    fn combine_messages(message_buffer: &mut Arc<Mutex<VecDeque<MessageBuffer>>>) -> MessageBuffer {

        let mut message_buffer = message_buffer.lock().unwrap();
        let to_send = message_buffer.pop_front().unwrap();

        match to_send {
            MessageBuffer::Newline(msg) => {
                let mut message = msg;
                let mut message_length = message.chars().count();
                while message_buffer.len() > 0 {

                    let new_pop = {
                        message_buffer.pop_front().unwrap()
                    };

                    if let MessageBuffer::Newline(msg) = new_pop {
                        // count the chars and not just String.len()
                        // as the limit is at 4096 utf8 chars defined by
                        // the telegram api and not 4096 bytes which would be
                        // String.len() >= 4096

                        let this_message_length = msg.chars().count();

                        if this_message_length + message_length + 1 >= 4096 {
                            message_buffer.push_front(MessageBuffer::Newline(msg));
                            break;
                        } else {
                            message.push('\n');
                            message.push_str(&msg);
                            message_length += this_message_length + 1;
                        }
                    }
                }

                return MessageBuffer::Newline(message);
            }
            MessageBuffer::CarriageReturn(msg) => return MessageBuffer::CarriageReturn(msg),
        }
    }


    // sends the given string if the message is longer than 0
    // if successfully sent, this returns a message id
    fn send(&mut self, s: String) {
        if s.len() > 0 {
            match self.api.send_message(self.user_id, s, None, None, None, None) {
                Ok(o) => self.last_sent_message = Some(o),
                Err(err) => print!("error while sending: {}", err),
            }
        }
    }

    // overrides the last message with the given string if the message is longer than 0
    // also the id of the last sent message
    // if this id is None, then nothing is done
    fn override_last(&mut self, s: String) {
        if s.len() > 0 {
            match self.last_sent_message.take() {
                Some(m) => {

                    // if trying to override last, but last is the same
                    // ignore this one
                    let mut is_same_message = false;
                    if let &telegram_bot::types::MessageType::Text(ref t) = &m.msg {
                        if t == &s {
                            is_same_message = true;
                        }
                    }

                    if is_same_message {
                        self.last_sent_message = Some(m);
                        return;
                    }

                    // get the old text that was sent
                    let old_text = if let &telegram_bot::types::MessageType::Text(ref t) = &m.msg {
                        t.clone()
                    } else {
                        String::new()
                    };

                    // split it by newlines
                    let mut parts = old_text.split("\n").collect::<Vec<&str>>();

                    // new when override last is called, the last \n part should be overriden
                    // so remove this
                    if parts.len() > 0 {
                        parts.pop();
                    }

                    // and push the new message there
                    parts.push(&s);

                    // glue everything back together
                    let final_message = parts.join("\n");

                    // and go
                    match self.api.edit_message_text(Some(m.chat.id()),
                                                     Some(m.message_id),
                                                     None,
                                                     final_message,
                                                     None,
                                                     None,
                                                     None) {
                        Ok(o) => self.last_sent_message = Some(o),
                        Err(err) => {
                            self.last_sent_message = Some(m);
                            println!("error while overriding {}", err);
                        }
                    }
                }
                None => println!("None message was given"),
            }
        }
    }
}

pub struct TeleechoProcessor {
    /// this is the input buffer
    /// this is different from the message buffer, as messages are the 
    /// split up input buffer, while the input buffer is the 
    /// raw input from the pipe
    input_buffer: String,

    /// keep account of how long the input buffer is
    input_buffer_size: usize,

    sender: Sender<BufferChangeEvent>,

    /// a buffer that stores the messages to be sent
    message_buffer: Arc<Mutex<VecDeque<MessageBuffer>>>,

    handle: Option<JoinHandle<()>>,
}

impl TeleechoProcessor {
    pub fn create(token: &str, user_id: i64) -> Result<TeleechoProcessor> {

        let api = try!(telegram_bot::Api::from_token(&token));

        let (sender, handle, buffer) = TeleechoSender::create(api, user_id);

        Ok(TeleechoProcessor {
            input_buffer: String::with_capacity(8000),
            input_buffer_size: 0,
            sender: sender,
            message_buffer: buffer.clone(),
            handle: Some(handle),
        })
    }

    /// if the send thread is still running this sends the kill signal 
    /// and waits for the thread to finish up
    /// if was already closed, nothing will be done
    pub fn close(&mut self) {
        match self.handle.take() {
            Some(handle) => {
                self.sender.send(BufferChangeEvent::Kill).unwrap();
                handle.join().unwrap();
            }
            None => {}
        }
    }

    /// given a MessageBuffer event this appends the message
    /// into the buffer. 
    /// if CarriageReturn and another message present this
    /// message is overriden
    fn append_to_send_buffer(&mut self, msg: MessageBuffer) {

        let mut msg_buffer = self.message_buffer.lock().unwrap();

        if msg_buffer.len() == 0 {
            msg_buffer.push_back(msg);
        } else if let &MessageBuffer::Newline(_) = &msg {
            msg_buffer.push_back(msg);
        } else if let MessageBuffer::CarriageReturn(s) = msg {
            // get last element; will exist, as len() > 0
            let last_elem = msg_buffer.pop_back().unwrap();

            let new_elem = match last_elem {
                MessageBuffer::CarriageReturn(_) => MessageBuffer::CarriageReturn(s),
                MessageBuffer::Newline(_) => MessageBuffer::Newline(s),
            };

            msg_buffer.push_back(new_elem);
        }

        self.sender.send(BufferChangeEvent::NewElement).unwrap();
    }

    /// appends the given string to the input buffer
    pub fn append_to_input_buffer(&mut self, c: char) {

        if c == '\n' || c == '\r' {
            self.convert_to_message();
        }

        // add all chars, even '\r' but not '\n' as this
        // triggers flush
        // '\r' is needed to know whether one has to override
        // the last message
        if c != '\n' {
            self.input_buffer.push(c);
            self.input_buffer_size += 1;
        }

        // if after adding the buffer is too big, flush
        if self.input_buffer_size >= 4096 {
            self.convert_to_message();
        }
    }

    /// call this when '\r', '\n' or 4096 chars are reached
    /// this then converts this to a message
    /// and appends this to the input buffer
    fn convert_to_message(&mut self) {

        // this will hold the message text
        let mut message_text = String::with_capacity(self.input_buffer.len());

        // try to find out if there is a carriage return in the message
        // if it is, then it means the carriage return must be the first
        // character
        let mut is_carriage_return = false;
        for ch in self.input_buffer.chars() {
            if ch == '\r' {
                is_carriage_return = true;
            } else {
                message_text.push(ch);
            }
        }

        // compose the message
        let message = if is_carriage_return {
            MessageBuffer::CarriageReturn(message_text)
        } else {
            MessageBuffer::Newline(message_text)
        };

        // send
        self.append_to_send_buffer(message);

        // clear buffer and size
        self.input_buffer.clear();
        self.input_buffer_size = 0;
    }
}

// implement drop for the processor to
// prevent forgetting to call close
impl Drop for TeleechoProcessor {
    fn drop(&mut self) {
        self.close();
    }
}

/// given a token this starts a listener for telegram messages.
/// if the randomly generated pairing number is send via telegram
/// to this bot a new connection pair is returned
/// if something goes wrong an Error is returned
pub fn register_connection(token: &str) -> Result<(String, i64)> {

    let api = try!(telegram_bot::Api::from_token(&token));
    let me = try!(api.get_me());

    // generate a random number to be used for pairing
    // its probably possible to just use the "/start" command
    let mut rng = rand::thread_rng();
    let random_number = rng.gen_range(0, 99999);

    println!("send the following number to the {} bot:\t{}",
             me.username.unwrap(),
             random_number);

    // this will hold the user id to send the messages to
    let mut user_id = None;

    // create the listener and listen what the user has to say
    let mut listener = api.listener(telegram_bot::ListeningMethod::LongPoll(None));
    try!(listener.listen(|u| {
        // If the received update contains a message...
        if let Some(m) = u.message {
            let name = m.from.first_name;

            // Match message type
            match m.msg {
                telegram_bot::MessageType::Text(t) => {

                    // if the corret number was specified
                    if t == format!("{}", random_number) {

                        // notify the user
                        // but dont panic if this did not work
                        match api.send_message(m.chat.id(),
                                               String::from("correct number!"),
                                               None,
                                               None,
                                               None,
                                               None) {
                            Ok(_) => {}
                            Err(err) => println!("Error while register {}", err),
                        };

                        user_id = Some(m.chat.id());
                        return Ok(telegram_bot::ListeningAction::Stop);

                    } else {
                        println!("received wrong number from {}", name);
                    }
                }
                _ => {}
            }
        }

        // If none of the "try!" statements returned an error: It's Ok!
        Ok(telegram_bot::ListeningAction::Continue)
    }));

    // the user id must have been found, otherwise the connection is not complete
    if user_id.is_none() {
        Err("user id is empty".into())
    } else {
        Ok((String::from(token), user_id.unwrap()))
    }
}

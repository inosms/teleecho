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
    message_buffer: Arc<Mutex<Vec<MessageBuffer>>>,

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
                  Arc<Mutex<Vec<MessageBuffer>>>) {

        // create the sender object
        let ts = TeleechoSender {
            last_sent_message: None,
            api: api,
            message_buffer: Arc::new(Mutex::new(vec![])),
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
                        thread::sleep(::std::time::Duration::from_millis((1000000000u64 -
                                                                          time_diff) /
                                                                         1000000u64));
                    }

                    // if a new message event is received this does not mean, that
                    // the buffer still has a message, as with the last message event this
                    // message could also have been sent already, as the messages get combined
                    if ts.message_buffer.lock().unwrap().len() > 0 {

                        let to_send = TeleechoSender::combine_messages(&mut ts.message_buffer);

                        ts.last_send_time = time::precise_time_ns();

                        match to_send {
                            MessageBuffer::Newline(msg) => ts.send(msg),
                            MessageBuffer::CarriageReturn(msg) => ts.override_last(msg),
                        }
                    }
                }
            }
        }
    }

    fn combine_messages(message_buffer: &mut Arc<Mutex<Vec<MessageBuffer>>>) -> MessageBuffer {
        let to_send = message_buffer.lock().unwrap().remove(0);

        match to_send {
            MessageBuffer::Newline(msg) => {
                let mut message = msg;
                while message_buffer.lock().unwrap().len() > 0 {

                    let new_pop = {
                        message_buffer.lock().unwrap().remove(0)
                    };

                    if let MessageBuffer::Newline(msg) = new_pop {
                        if msg.len() + message.len() >= 4096 {
                            message_buffer.lock()
                                          .unwrap()
                                          .insert(0, MessageBuffer::Newline(msg));
                            break;
                        } else {
                            message.push('\n');
                            message.push_str(&msg);
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
                    match self.api.edit_message_text(m.chat.id(),
                                                     m.message_id,
                                                     final_message,
                                                     None,
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

    sender: Sender<BufferChangeEvent>,

    /// a buffer that stores the messages to be sent
    message_buffer: Arc<Mutex<Vec<MessageBuffer>>>,

    handle: Option<JoinHandle<()>>,
}

impl TeleechoProcessor {
    pub fn create(token: &str, user_id: i64) -> Result<TeleechoProcessor> {

        let api = try!(telegram_bot::Api::from_token(&token));

        let (sender, handle, buffer) = TeleechoSender::create(api, user_id);

        Ok(TeleechoProcessor {
            input_buffer: String::new(),
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
            msg_buffer.push(msg);
        } else if let &MessageBuffer::Newline(_) = &msg {
            msg_buffer.push(msg);
        } else if let MessageBuffer::CarriageReturn(s) = msg {
            // get last element; will exist, as len() > 0
            let last_elem = msg_buffer.pop().unwrap();

            let new_elem = match last_elem {
                MessageBuffer::CarriageReturn(_) => MessageBuffer::CarriageReturn(s),
                MessageBuffer::Newline(_) => MessageBuffer::Newline(s),
            };

            msg_buffer.push(new_elem);
        }

        self.sender.send(BufferChangeEvent::NewElement).unwrap();
    }

    /// appends the given string to the input buffer
    pub fn append_to_input_buffer(&mut self, s: &str) {
        // append the input the the buffer
        self.input_buffer.push_str(s);

        // split the buffer at '\r', '\n' or 4096 chars
        self.split_input();
    }

    /// splits the buffer at '\r', '\n' or 4096 chars
    /// then appends the parts to the send buffer
    fn split_input(&mut self) {

        let mut result_buffer = vec![];
        {
            let mut i = 0;
            let mut starts_with_r = false;

            let mut buffer = String::with_capacity(::std::cmp::min(self.input_buffer.len(), 4096));

            for c in self.input_buffer.chars() {

                if i == 0 && c == '\r' {
                    starts_with_r = true;
                    buffer.push(c);
                } else if c == '\r' || c == '\n' || i >= 4096 {
                    if starts_with_r {
                        result_buffer.push(MessageBuffer::CarriageReturn(buffer.replace("\r", "")));
                    } else {
                        result_buffer.push(MessageBuffer::Newline(buffer));
                    }

                    buffer = String::with_capacity(::std::cmp::min(self.input_buffer.len(), 4096));
                    i = -1;
                    starts_with_r = false;

                    if c == '\r' {
                        starts_with_r = true;
                        buffer.push(c);
                    }
                } else {
                    buffer.push(c);
                }

                i += 1;
            }

            self.input_buffer = buffer;
        }

        for r in result_buffer {
            self.append_to_send_buffer(r);
        }
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

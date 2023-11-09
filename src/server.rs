use std::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::Result;
use automerge::{
    sync::{self, Message, State, SyncDoc},
    AutoCommit, PatchLog,
};

use crate::{receive_bytes, send_n_bytes};

fn handle_connection(
    mut stream: TcpStream,
    sender: Sender<BroadcasterAction>,
    index: usize,
) -> Result<()> {
    let mut buffer = vec![];
    loop {
        receive_bytes(&mut stream, &mut buffer)?;
        let message = Message::decode(&buffer)?;
        sender.send(BroadcasterAction::MessageReceived {
            message,
            source_index: index,
        })?;
    }
}

enum BroadcasterAction {
    NewClient(TcpStream),
    MessageReceived {
        message: Message,
        source_index: usize,
    },
}

struct Broadcaster {
    writer_streams: Vec<TcpStream>,
    sync_states: Vec<State>,
    doc: AutoCommit,
    sender: Sender<BroadcasterAction>,
}

impl Broadcaster {
    fn new_client(&mut self, stream: TcpStream) -> Result<()> {
        self.writer_streams.push(stream.try_clone()?);
        self.sync_states.push(sync::State::new());
        let id = self.sync_states.len() - 1;

        let cloned = self.sender.clone();
        self.sync_client(id)?;

        thread::spawn(move || {
            let result = handle_connection(stream, cloned, id);
            print!("Thread {} exited because of {:?}", id, result.unwrap_err())
        });
        Ok(())
    }

    fn message_received(&mut self, message: Message, source_index: usize) {
        let mut patch_log = PatchLog::new(true, automerge::patches::TextRepresentation::String);
        self.doc
            .sync()
            .receive_sync_message_log_patches(
                &mut self.sync_states[source_index],
                message,
                &mut patch_log,
            )
            .unwrap();
        println!("Received message with patches {:?}", patch_log);
        for id in 0..self.sync_states.len() {
            let r = self.sync_client(id);
            if let Err(err) = r {
                println!("error synchronizing client {}: {:?}", id, err);
            }
        }
    }

    fn sync_client(&mut self, id: usize) -> Result<()> {
        let message = self
            .doc
            .sync()
            .generate_sync_message(&mut self.sync_states[id]);

        if message.is_none() {
            return Ok(());
        }
        let message = message.unwrap();
        send_n_bytes(&mut self.writer_streams[id], &message.encode())?;
        Ok(())
    }
}

fn work_loop(work_queue: Receiver<BroadcasterAction>, broadcaster: &mut Broadcaster) {
    for message in work_queue {
        match message {
            BroadcasterAction::NewClient(stream) => {
                let result = broadcaster.new_client(stream);
                println!("Added new client with result {:?}", result);
            }
            BroadcasterAction::MessageReceived {
                message,
                source_index,
            } => {
                broadcaster.message_received(message, source_index);
            }
        }
    }
}

pub fn server(port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let (tx, rx) = mpsc::channel();
    let new_client_sender = tx.clone();

    thread::spawn(move || {
        let mut bc = Broadcaster {
            writer_streams: vec![],
            sync_states: vec![],
            doc: AutoCommit::new(),
            sender: tx,
        };
        work_loop(rx, &mut bc);
    });

    for stream in listener.incoming() {
        new_client_sender.send(BroadcasterAction::NewClient(stream?))?;
    }
    Ok(())
}

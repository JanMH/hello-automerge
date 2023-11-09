use std::{
    net::TcpStream,
    ops::DerefMut,
    sync::{Arc, Mutex},
    thread,
};

use automerge::{
    sync::{Message, State, SyncDoc},
    AutoCommit, ChangeHash, PatchLog,
};

use anyhow::Result;
use autosurgeon::{hydrate, reconcile, Hydrate, Reconcile};

use crate::{receive_bytes, send_n_bytes};

#[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
struct Contact {
    name: String,
}

pub fn client(port: u16) {
    let mut c = Client::connect(&format!("127.0.0.1:{}", port)).unwrap();

    let stdin = std::io::stdin(); // We get `Stdin` here.
    loop {
        let mut buffer = String::new();

        stdin.read_line(&mut buffer).unwrap();
        match buffer.as_str().trim() {
            "get" => {
                get_data(&mut c);
            }
            "set" => {
                set_data(&mut c);
            }
            unknown => {
                println!("Unknown command {}", unknown)
            }
        }
    }
}

fn get_data(c: &mut Client) {
    let contact: Result<Contact, _> = c.hydrate();
    match contact {
        Ok(contact) => {
            println!("{:?}", &contact)
        }
        Err(err) => {
            println!("Could not load contact {:?}", &err)
        }
    }
}

fn set_data(c: &mut Client) {
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();

    let contact = Contact { name: buf };
    c.reconcile(contact).unwrap();
    c.upload_local_changes().unwrap();
}

fn receive_loop(mut stream: TcpStream, sync_info: Arc<Mutex<(AutoCommit, State)>>) -> Result<()> {
    let mut buffer = vec![];

    loop {
        println!("Receiving bytes...");
        receive_bytes(&mut stream, &mut buffer)?;
        println!("...Received bytes");

        let message = Message::decode(&buffer)?;
        {
            let mut sync_info = sync_info.lock().unwrap();
            let (doc, state) = sync_info.deref_mut();
            let mut patch_log = PatchLog::new(true, automerge::patches::TextRepresentation::String);
            doc.sync()
                .receive_sync_message_log_patches(state, message, &mut patch_log)?;
            println!("Applying patches locally {:?}", patch_log);
        }
    }
}

pub struct Client {
    stream: TcpStream,
    sync_info: Arc<Mutex<(AutoCommit, State)>>,
}

impl Client {
    pub fn connect(address: &str) -> Result<Client> {
        let stream = TcpStream::connect(address)?;
        let sync_info = (AutoCommit::new(), State::new());
        let sync_info = Arc::new(Mutex::new(sync_info));
        let mut client = Client {
            stream: stream.try_clone()?,
            sync_info: sync_info.clone(),
        };
        client.upload_local_changes()?;
        thread::spawn(|| {
            println!(
                "Receiveloop exited with {:?}",
                receive_loop(stream, sync_info).unwrap_err()
            );
        });
        Ok(client)
    }

    pub fn upload_local_changes(&mut self) -> Result<()> {
        let message = {
            let mut sync_info = self.sync_info.lock().unwrap();
            let (doc, state) = sync_info.deref_mut();
            let m = doc.sync().generate_sync_message(state);
            m
        };
        if message.is_none() {
            return Ok(());
        }
        let message = message.unwrap();
        send_n_bytes(&mut self.stream, &message.encode())
    }

    #[allow(unused)]
    pub fn fork(&mut self) -> Result<AutoCommit> {
        let mut sync_info = self.sync_info.lock().unwrap();
        Ok(sync_info.0.fork())
    }

    #[allow(unused)]
    pub fn merge(&mut self, doc: &mut AutoCommit) -> Result<Vec<ChangeHash>> {
        let mut sync_info = self.sync_info.lock().unwrap();
        let r = sync_info.0.merge(doc)?;
        doc.merge(&mut sync_info.0)?;

        Ok(r)
    }

    pub fn hydrate<T: Hydrate>(&mut self) -> Result<T> {
        let mut sync_info = self.sync_info.lock().unwrap();
        let r = hydrate(&mut sync_info.0)?;
        Ok(r)
    }

    pub fn reconcile<T: Reconcile>(&mut self, data: T) -> Result<()> {
        let mut sync_info = self.sync_info.lock().unwrap();
        reconcile(&mut sync_info.0, data)?;
        Ok(())
    }
}

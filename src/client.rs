use std::{
    net::TcpStream,
    ops::DerefMut,
    sync::{Arc, Mutex}, thread,
};

use automerge::{
    sync::{Message, State, SyncDoc},
    AutoCommit, hydrate, ChangeHash,
};

use anyhow::Result;
use autosurgeon::Hydrate;

use crate::{receive_bytes, send_n_bytes};

pub fn receive_loop(
    mut stream: TcpStream,
    sync_info: Arc<Mutex<(AutoCommit, State)>>,
) -> Result<()> {
    let mut buffer = vec![];

    loop {
        receive_bytes(&mut stream, &mut buffer)?;
        let message = Message::decode(&buffer)?;
        {
            let mut sync_info = sync_info.lock().unwrap();
            let (doc, state) = sync_info.deref_mut();
            doc.sync().receive_sync_message(state, message)?;
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
        let client = Client {
            stream: stream.try_clone()?,
            sync_info: sync_info.clone()
        };
        thread::spawn(|| {
            println!("Receiveloop exited with {:?}", receive_loop(stream, sync_info).unwrap_err());
        });
        Ok(client)
    }

    pub fn sync_client(&mut self) -> Result<()> {
        let message = {
            let mut sync_info = self.sync_info.lock().unwrap();
            let (doc, state) = sync_info.deref_mut();
            let m = doc.sync().generate_sync_message( state);
            m
        };
        if message.is_none() {
            return Ok(());
        }
        let message = message.unwrap();
        send_n_bytes(&mut self.stream, &message.encode())
    }

    pub fn fork(&mut self) -> Result<AutoCommit> {
        let mut sync_info = self.sync_info.lock().unwrap();
        Ok(sync_info.0.fork())
    }

    pub fn merge(&mut self, doc: &mut AutoCommit) -> Result<Vec<ChangeHash>> {
        let mut sync_info = self.sync_info.lock().unwrap();
        let r  = sync_info.0.merge(doc)?;
        Ok(r)
    }
}

mod server;
mod client;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{mpsc::{self, Receiver, Sender}, Arc, Mutex},
    thread, ops::DerefMut, env::args,
};

use anyhow::Result;
use automerge::{
    sync::{self, Message, State, SyncDoc},
    AutoCommit, Automerge,
};
use autosurgeon::{hydrate, reconcile, Hydrate, Reconcile};
use client::Client;
use server::server;

fn send_n_bytes(stream: &mut TcpStream, bytes: &[u8]) -> Result<()> {
    stream.write_all(&(bytes.len() as u64).to_le_bytes())?;
    stream.write_all(bytes)?;
    Ok(())
}

fn receive_bytes(stream: &mut TcpStream, buffer: &mut Vec<u8>) -> Result<()> {
    buffer.resize(std::mem::size_of::<u64>(), 0);
    stream.read_exact(buffer)?;
    let len = u64::from_le_bytes(buffer.as_slice().try_into().unwrap());
    buffer.resize(len as usize, 0);
    stream.read_exact(buffer)?;
    Ok(())
}

fn print_usage()  {
    println!("usage: hello-automerge <client|server>")
}

#[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
struct Contact {
    name: String
}

fn client() {
    let mut c = Client::connect("localhost:666").unwrap();
    let mut doc = c.fork().unwrap().with_actor(automerge::ActorId::random());

    let mut buffer = String::new();
    let stdin = std::io::stdin(); // We get `Stdin` here.
    loop {
        stdin.read_line(&mut buffer).unwrap();
        match buffer.as_str() {
            "get" =>{
                get_data(&mut c, &mut doc);
                },
            "set" => {
set_data(&mut c, &mut doc);
            },
            _ => {}
        }
    }
}

fn get_data(c: &mut Client, doc: &mut AutoCommit) {
    c.merge(doc).unwrap();
    let contact: Result<Contact, _> = hydrate(doc);
    match contact {
        Ok(contact) => {
            println!("{:?}", &contact)
        },
        Err(err) => {
            println!("Could not load contact {:?}", &err)

        }
    }
}

fn set_data(c: &mut Client, doc: &mut AutoCommit) {
    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf).unwrap();

    let contact = Contact{name: buf};
    reconcile(doc,contact).unwrap();
    c.merge(doc).unwrap();
    c.sync_client().unwrap();
}


fn main() {
    
    let mode = match args().nth(1) {
        Some(mode) => mode,
        None => {
            print_usage();
            return
        }
    };
    match mode.as_str() {
        "server" => server(666).unwrap(),
        "client" => client(),
        _ => {
            print_usage();
        }
    };
    // let mut contact = Contact {
    //     name: "Sherlock Holmes".to_string(),
    //     address: Address {
    //         line_one: "221B Baker St".to_string(),
    //         line_two: None,
    //         city: "London".to_string(),
    //         postcode: "NW1 6XE".to_string(),
    //     },
    // };

    // // Put data into a document
    // let mut doc = automerge::AutoCommit::new();
    // reconcile(&mut doc, &contact).unwrap();

    // // Get data out of a document
    // let contact2: Contact = hydrate(&doc).unwrap();
    // assert_eq!(contact, contact2);

    // // Fork and make changes
    // let mut doc2 = doc.fork().with_actor(automerge::ActorId::random());
    // let mut contact2: Contact = hydrate(&doc2).unwrap();
    // contact2.name = "Dangermouse".to_string();
    // reconcile(&mut doc2, &contact2).unwrap();

    // // Concurrently on doc1
    // contact.address.line_one = "221C Baker St".to_string();
    // reconcile(&mut doc, &contact).unwrap();

    // // Now merge the documents
    // // Reconciled changes will merge in somewhat sensible ways
    // doc.merge(&mut doc2).unwrap();

    // let merged: Contact = hydrate(&doc).unwrap();
    // doc.diff_incremental();
    // assert_eq!(
    //     merged,
    //     Contact {
    //         name: "Dangermouse".to_string(), // This was updated in the first doc
    //         address: Address {
    //             line_one: "221C Baker St".to_string(), // This was concurrently updated in doc2
    //             line_two: None,
    //             city: "London".to_string(),
    //             postcode: "NW1 6XE".to_string(),
    //         }
    //     }
    // )
}

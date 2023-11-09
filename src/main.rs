mod client;
mod server;
use std::{
    env::args,
    io::{Read, Write},
    net::TcpStream,
};

use anyhow::Result;

use client::client;
use server::server;

const PORT: u16 = 6666;

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

fn print_usage() {
    println!("usage: hello-automerge <client|server>")
}

fn main() {
    let mode = match args().nth(1) {
        Some(mode) => mode,
        None => {
            print_usage();
            return;
        }
    };
    match mode.as_str() {
        "server" => server(PORT).unwrap(),
        "client" => client(PORT),
        _ => {
            print_usage();
        }
    };
}

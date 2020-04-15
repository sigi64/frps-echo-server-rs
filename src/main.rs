//extern crate async_listen;
extern crate async_std;
extern crate libfrps_rs;
extern crate http_types;

//use async_listen::{backpressure, error_hint, ByteStream, ListenExt};
use async_std::net::{TcpListener, TcpStream, ToSocketAddrs};
use async_std::prelude::*;
use async_std::task;
use http_types::{Response, StatusCode};
use std::io;
use std::time::Duration;
use libfrps_rs::{Serializer, Tokenizer, Value, ValueTreeBuilder};

#[async_std::main]
async fn main() -> http_types::Result<()> {
    // Open up a TCP connection and create a URL.
    let listener = TcpListener::bind(("127.0.0.1", 8080)).await?;
    let addr = format!("http://{}", listener.local_addr()?);
    println!("listening on {}", addr);

    // For each incoming TCP connection, spawn a task and call `accept`.

    // TODO: try async-listern crate to handle errors
    // (connection reset by peer, too many open files)
    // and limit number of incomming connections
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let addr = addr.clone();
        task::spawn(async {
            if let Err(err) = accept(addr, stream).await {
                eprintln!("{}", err);
            }
        });
    }
    Ok(())
}

// Take a TCP stream, and convert it into sequential HTTP request / response pairs.
async fn accept(addr: String, stream: TcpStream) -> http_types::Result<()> {
    println!("starting new connection from {}", stream.peer_addr()?);
    async_h1::accept(&addr, stream.clone(), |req| async move {
        if let Some(val) = req.header("User-agent") {
            println!("User-agent: {}", val)
        }

        if Some(val) = req.header("Accept") {
            println!("Accept: {}", val)
        };

        if Some(val) = req.header("Content-Type") {
            println!("Content-Type: {}", val)
        };
            
        let ct = req.header(name);

        let mut res = Response::new(StatusCode::Ok);
        res.insert_header("Content-Type", "text/plain")?;
        res.set_body("Hello");
        Ok(res)
    })
    .await?;

    Ok(())
}

// fn log_accept_error(e: &io::Error) {
//     eprintln!("Error: {}. Listener paused for 0.5s. {}", e, error_hint(e));
// }

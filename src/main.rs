//extern crate async_listen;
extern crate async_std;
extern crate http_types;
extern crate libfrps_rs;

//use async_listen::{backpressure, error_hint, ByteStream, ListenExt};
use async_std::net::{TcpListener, TcpStream, ToSocketAddrs};
use async_std::prelude::*;
use async_std::task;
use async_std::io::{BufRead, Read};
use http_types::headers::{HeaderName, HeaderValue};
use http_types::{Body, Headers, Response, StatusCode};
use libfrps_rs::{Serializer, Tokenizer, Value, ValueTreeBuilder};
use std::io;
use std::time::Duration;
use std::str::FromStr;

#[async_std::main]
async fn main() -> http_types::Result<()> {
    // Open up a TCP connection and create a URL.
    let listener = TcpListener::bind(("127.0.0.1", 30001)).await?;
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

        let content_length = HeaderName::from_str("content-length").unwrap();
        let content_type = HeaderName::from_str("content-type").unwrap();
        let accept = HeaderName::from_str("accept").unwrap();
        let user_agent = HeaderName::from_str("user-agent").unwrap();
        
        if let Some(val) = req.header(&user_agent) {
            println!("User-agent: {}", val[0])
        }

        // Get type of protocol
        let mut is_frps = false;
        if let Some(val) = req.header(&content_type) {
            let val = &val[0];
            println!("Content-type: {}", val);

            match val.as_str() {
                "application/x-frpc" => is_frps = false,
                "application/x-frps" => is_frps = true,
                _ => {
                    let mut res = Response::new(StatusCode::BadRequest);
                    res.insert_header("Content-Type", "text/plain")?;
                    res.set_body("Invalid content-type expected: either application/x-frpc or application/x-frps");
                    return Ok(res)
                } 
            }
        }

        if let Some(val) = req.header(&content_length) {
            println!("Content-lenght: {}", val[0])
        }

        // Get type of response supported by client. We prefer frps
        let mut response_as_frps = is_frps;
        if let Some(val) = req.header(&accept) {
            for v in val {
                println!("Accept: {}", v);
                if v.as_str().contains("application/x-frps") {
                    response_as_frps = true;
                    break;
                }
            }
        }

        let body: Body = req.into();
        let body: String = body.into_string().await?;

        // parse
        let mut tokenizer = if is_frps {
            Tokenizer::new_frps()
        } else {
            Tokenizer::new_frpc()
        };

        let mut values = ValueTreeBuilder::new();

        match tokenizer.parse(body.as_bytes(), &mut values) {
            OK(finished, len) => {},
            Err(_) => {
                let mut res = Response::new(StatusCode::BadRequest);
                res.insert_header("Content-Type", "text/plain")?;
                res.set_body("Invalid body, can't deserialize");
                return Ok(res)
            }
        }

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

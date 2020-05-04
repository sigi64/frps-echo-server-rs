//use async_listen::{backpressure, error_hint, ByteStream, ListenExt};
use async_std::prelude::*;
use async_std::net::{TcpListener, TcpStream, ToSocketAddrs};
use async_std::task;
use http_types::headers::{HeaderName, HeaderValue};
use http_types::{Body, Headers, Response, StatusCode};
use libfrps_rs::{ParsedStatus, Serializer, Tokenizer, Value, ValueTreeBuilder};
use std::str::FromStr;
use std::time::Duration;

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
        let mut reader = body.into_reader();
        // let mut data = reader.bytes();
        // while let Some(byte) = data.next().await {
        //     println!("Data:{}", byte?);
        // }

        // parse
        let mut tokenizer = if is_frps {
            Tokenizer::new_frps()
        } else {
            Tokenizer::new_frpc()
        };

        let mut call = ValueTreeBuilder::new();
        // keep state of processing incoming data
        let mut data_after_end = false;
        let mut need_data = true; // we are greedy for data

        let mut b: [u8; 1024] = [0; 1024];
        while let Ok(len) = reader.read(&mut b).await {
            if len == 0 {
                break;
            }
            println!("Body chunk:{}", len);

            match tokenizer.parse(&b[..len], &mut call) {
                Ok((expecting_data, processed)) => {
                    need_data = expecting_data;
                    if !expecting_data {
                        // propagate if tokenizer parsed all data
                        data_after_end = processed < 1;
                        if data_after_end {
                            break;
                        }
                    }
                },
                Err(_) => {
                    let mut res = Response::new(StatusCode::BadRequest);
                    res.insert_header("Content-Type", "text/plain")?;
                    res.set_body("Invalid body, can't deserialize");
                    return Ok(res)
                }
            }
        }

        let mut response = Response::new(StatusCode::Ok);
        response.insert_header("User-Agent", "frps-echo-server-rs");
        response.insert_header("Content-Type", "text/plain")?;
    
        let body = if need_data {
            String::from("error(unexpected data end)")
        } else if data_after_end {
            String::from("error(data after end)")
        } else {
            // ok response
            format!("{}", call)
        };
    
        println!("{}", call);

        response.set_body(body);
        Ok(response)
    })
    .await?;

    Ok(())
}

fn create_echo_response(call: &ValueTreeBuilder) -> Vec<u8> {
    use enum_extract::let_extract;

    let mut serializer = Serializer::new();
    let mut buffer = vec![];
    // now try to serialize
    buffer.resize(1024, 0); // make buffer large enought
    let mut cnt: usize = 0;
    match &call.what {
        ParsedStatus::Fault => {
            assert!(
                call.values.len() == 2,
                "There should be Fault with 2 values"
            );
            let_extract!(Value::Int(code), &call.values[0], unreachable!());
            let_extract!(Value::Str(msg), &call.values[1], unreachable!());
            let r = serializer.write_fault(&mut buffer[cnt..], *code, msg.as_str());
            cnt += r.unwrap();
        }
        ParsedStatus::MethodCall(name) => {
            let r = serializer.write_call(&mut buffer[cnt..], name.as_str());
            cnt += r.unwrap();
            for i in 0..call.values.len() {
                serializer.reset();
                let r = serializer.write_value(&mut buffer[cnt..], &call.values[i]);
                cnt += r.unwrap();
            }
        }
        ParsedStatus::Response => {
            assert!(call.values.len() == 1);
            let r = serializer.write_response(&mut buffer[cnt..], &call.values[0]);
            cnt += r.unwrap();
        }
        _ => return vec![],
    }
    if !call.data.is_empty() {
        serializer.reset();
        let r = serializer.write_data(&mut buffer[cnt..], &call.data);
        cnt += r.unwrap();
    }

    return buffer;
}

// fn log_accept_error(e: &io::Error) {
//     eprintln!("Error: {}. Listener paused for 0.5s. {}", e, error_hint(e));
// }

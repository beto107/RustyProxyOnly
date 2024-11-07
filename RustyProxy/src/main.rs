use std::io::{Error, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::Duration;
use std::{env, thread};

fn main() {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", get_port())).unwrap();
    start_http(listener);
}

fn start_http(listener: TcpListener) {
    for stream in listener.incoming() {
        if let Ok(client_stream) = stream {
            thread::spawn(move || handle_client(client_stream));
        } else {
            eprintln!("Error accepting connection");
        }
    }
}

fn handle_client(client_stream: TcpStream) {
    let status = get_status();
    if client_stream.write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes()).is_err() {
        return;
    }

    if let Ok(data_str) = peek_stream(&client_stream) {
        if data_str.contains("HTTP") {
            let _ = client_stream.read(&mut vec![0; 1024]); // Descartar dados
            if data_str.to_lowercase().contains("websocket") {
                let _ = client_stream.write_all(format!("HTTP/1.1 200 {}\r\n\r\n", status).as_bytes());
            }
        }
    }

    let addr_proxy = determine_proxy_address(&client_stream);
    if let Ok(server_stream) = TcpStream::connect(addr_proxy) {
        transfer_data(client_stream, server_stream);
    }
}

fn transfer_data(mut client_stream: TcpStream, mut server_stream: TcpStream) {
    let (client_read, client_write) = (client_stream.try_clone().unwrap(), client_stream);
    let (server_read, server_write) = (server_stream.try_clone().unwrap(), server_stream);

    thread::spawn(move || transfer(&client_read, &server_write));
    transfer(&server_read, &client_write);
}

fn transfer(read_stream: &TcpStream, write_stream: &TcpStream) {
    let mut buffer = [0; 2048];
    loop {
        match read_stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if write_stream.write_all(&buffer[..n]).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = write_stream.shutdown(Shutdown::Both);
}

fn peek_stream(read_stream: &TcpStream) -> Result<String, Error> {
    let mut peek_buffer = vec![0; 1024];
    let bytes_peeked = read_stream.peek(&mut peek_buffer)?;
    let data = &peek_buffer[..bytes_peeked];
    Ok(String::from_utf8_lossy(data).to_string())
}

fn determine_proxy_address(client_stream: &TcpStream) -> &'static str {
    let (tx, rx) = mpsc::channel();
    let clone_client = client_stream.try_clone().unwrap();

    thread::spawn(move || {
        let result = peek_stream(&clone_client);
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_secs(1)) {
        Ok(Ok(data_str)) if data_str.contains("SSH") => "0.0.0.0:22",
        _ => "0.0.0.0:1194",
    }
}

fn get_port() -> u16 {
    env::args()
        .skip(1)
        .find_map(|arg| {
            if arg == "--port" {
                arg.parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(80)
}

fn get_status() -> String {
    env::args()
        .skip(1)
        .find_map(|arg| {
            if arg == "--status" {
                Some(arg)
            } else {
                None
            }
        })
        .unwrap_or_else(|| String::from("@RustyManager"))
}
extern crate futures;
extern crate tokio_core;
extern crate tokio_line;

use futures::future::Future;
use futures::sync::mpsc::UnboundedSender;
use futures::{Stream, Sink};
use futures::IntoFuture;
use std::io::{self, ErrorKind};
use tokio_core::io::Io;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Handle;
use tokio_line::LineCodec;
use tokio_core::reactor::Core;
use std::io::Write;
use std::fs::OpenOptions;

fn main () {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let address = "127.0.0.1:8765".parse().unwrap();
    let listener = TcpListener::bind(&address, &handle).unwrap();
    let connections = listener.incoming();

    let server = connections.for_each(move |(socket, _)| {
        let transport = socket.framed(LineCodec);
        let process_connection = transport.for_each(move |line| {
            println!("{}", line);

            let mut file = OpenOptions::new().append(true).open("/tmp/fake-server.txt").unwrap();
            file.write_all((line.to_string() + "\n").as_bytes());

            Ok(())
        })
        .map_err(|_| ());

        handle.spawn(process_connection);

        Ok(())
    });

    core.run(server).unwrap();
}

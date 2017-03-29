use futures::future::Future;
use futures::sync::mpsc::{UnboundedSender};
use futures::{Stream, Sink};
use std::io::{self, ErrorKind};
use std;
use super::Conf;
use tokio_core::io::{Io};
use tokio_core::net::{TcpListener};
use tokio_core::reactor::{Handle};
use tokio_line::LineCodec;

pub struct Server {
    configuration: Conf,
    handle: Handle,
    buftx: UnboundedSender<String>,
}

impl Server {
    pub fn new(configuration: Conf, handle: Handle, buftx: UnboundedSender<String>) -> Self {
        Server {
            configuration: configuration,
            handle: handle,
            buftx: buftx,
        }
    }

    pub fn accept_connection(self) -> Box<Future<Item = (), Error = std::io::Error>>{
        let address = self.configuration.listen_on.parse().unwrap();
        let listener = TcpListener::bind(&address, &self.handle).unwrap();
        let connections = listener.incoming();

        let server = connections.for_each(move |(socket, _)| {
            let transport = socket.framed(LineCodec);

            let nonhocapitoperchedevoclonarlo = self.buftx.clone();
            let process_connection = transport.for_each(move |line| {
                nonhocapitoperchedevoclonarlo.clone().send(line)
                    .map_err(|err| io::Error::new(ErrorKind::Other, err))
                    .map(|_| ())
            });

            self.handle.spawn(process_connection.map_err(|_| ()));

            Ok(())
        });

        Box::new(server)
    }
}

use futures::future::Future;
use futures::sync::mpsc::UnboundedSender;
use futures::{Stream, Sink};
use std::io::{self, ErrorKind};
use std::time::Duration;
use std;
use super::Conf;
use tokio_core::io::Io;
use tokio_core::net::TcpListener;
use tokio_core::reactor::Handle;
use tokio_line::LineCodec;
use tokio_timer::*;

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

    #[cfg(not(any(fake_clients)))]
    pub fn accept_connection(self) -> Box<Future<Item = (), Error = std::io::Error>> {
        let address = self.configuration.listen_on.parse().unwrap();
        let listener = TcpListener::bind(&address, &self.handle).unwrap();
        let connections = listener.incoming();

        let server = connections.for_each(move |(socket, _)| {
            let transport = socket.framed(LineCodec);

            let buftx = self.buftx.clone();
            let process_connection = transport.for_each(move |line| {
                buftx.clone().send(line)
                    .map_err(|err| io::Error::new(ErrorKind::Other, err))
                    .map(|_| ())
            })
            .map_err(|_| ());

            self.handle.spawn(process_connection);

            Ok(())
        });

        Box::new(server)
    }

    #[cfg(fake_clients)]
    pub fn accept_connection(self) -> Box<Future<Item = (), Error = TimerError>> {
        let timer = Timer::default();
        let wakeups = timer.interval(Duration::new(0, 150000000));
        let mut i = 0;
        let background_tasks = wakeups.for_each(move |_| {
            i = i + 1;
            debug!("Interval {}", i);
            self.buftx.clone()
                .send(format!(r#"{{"@timestamp":"2017-03-24T09:16:42.636040+01:00","@source":"dev-all-onebiptrusty cli","@fields":{{"channel":"integrationtest-client","level":100,"extra_level_name":"DEBUG","extra_uname":"dev-all-onebiptrusty","extra_sapi":"cli","extra_process_id":17954}},"@message":"Message {}"}}"#, i).to_string())
                .map(|_| ())
                .map_err(|_| TimerError::NoCapacity)
        });

        // let background_tasks = buftx.clone()
        //     .send(format!("Messagio {}", i).to_string())
        //     .map(|_| ())
        //     .map_err(|_| ());

        // self.handle.spawn(background_tasks.map(|_| ()).map_err(|_| ()));

        Box::new(background_tasks)
    }
}

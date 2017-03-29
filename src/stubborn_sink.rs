use futures::future::Future;
use futures::{Async, AsyncSink, Poll, StartSend, Stream, Sink};
use futures::sync::mpsc::{self, UnboundedSender};
// use futures::stream::{SplitSink};
use tokio_core::io::{Io};
use tokio_core::net::{TcpStream, TcpStreamNew};
use tokio_core::reactor::{Handle};
use tokio_line::LineCodec;
use std::{self, io, str, fmt, thread, time};
use std::string::String;
use std::net::SocketAddr;

enum RemoteConnectionState {
    NotConnected,
    Connecting(TcpStreamNew),
    Connected(UnboundedSender<String>),
}

impl fmt::Display for RemoteConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            RemoteConnectionState::NotConnected => {
                write!(f, "NotConnected")
            }
            RemoteConnectionState::Connecting(_) => {
                write!(f, "Connecting")
            }
            RemoteConnectionState::Connected(_) => {
                write!(f, "Connected")
            }
        }
    }
}

pub struct StubbornSink {
    remote_addr: std::net::SocketAddr,
    status: RemoteConnectionState,
    handle: Handle,
}

impl StubbornSink {
    pub fn new(remote_addr: SocketAddr, handle: Handle) -> Self {
        StubbornSink {
            remote_addr: remote_addr,
            status: RemoteConnectionState::NotConnected,
            handle: handle,
        }
    }

    fn connection_attempt(&mut self) -> TcpStreamNew {
        TcpStream::connect(&self.remote_addr, &self.handle.clone())
    }

    /**
    * I have failed to pass &self here, because the `match` `Connecting` branch locks self.
    * TODO:! Try using &self again!
    */
    fn get_inner_sink(stream: TcpStream, handle: &Handle) -> UnboundedSender<String> {
        let (middleware_tx, middleware_rx) = mpsc::unbounded::<String>();

        let (sender, receiver) = stream.framed(LineCodec).split();

        /**
         * This is the only method that I have found to stop sending messages to the remote server after
         * it closed the connection, I don't like it, I would try to avoid the middleware_tx and
         * the reader future:
         *
         * - reading data from remote server, even if it doesn't send anything, and even if I do not need that data.
         *   This future ends properly when the connection is closed.
         * - create a middleware channel (unbound)
         * - send the data we would like to send to the remote server, to the middleware tx
         * - forward data received on the middleware_rx to the remoteServer
         * - link the reader future with the writer future, so that when the connection is closed the
         * reader future ends and so it stop also the writer future.
         * - spawn the linked future
         */
        let reader = receiver
            .for_each(|_message| {
                Ok(())
            })
            .and_then(|_| {
                info!("Connection with remote server is lost");
                Ok(())
            });

        let writer = middleware_rx
            .map_err(|_| panic!("middleware_rx should never fails"))
            .forward(sender)
            .and_then(|(_rx, _tx)| Ok(()))
        ;

        let linked_future = reader.select(writer)
            .map_err(|(_err, _nf)| ())
            .and_then(move |_| Ok(()));

        handle.spawn(linked_future);

        middleware_tx
    }
}

impl Sink for StubbornSink {
    type SinkItem = String;
    type SinkError = io::Error;

    fn start_send(&mut self, msg: String) -> StartSend<String, io::Error> {
        /**
         * I need a loop to handle current state and also the next state,
         * avoiding code duplication and recursion
         */
        loop {
            debug!("current status: {}", self.status);

            /**
             * current status cannot be updated "on the fly" because the enum is in "use"
             */
            let next_status = match self.status {
                RemoteConnectionState::Connected(ref split_sink) => {
                    match split_sink.send(msg.clone()) {
                        Ok(_) => return Ok(AsyncSink::Ready),
                        Err(_) => Some(RemoteConnectionState::NotConnected),
                    }
                }
                RemoteConnectionState::Connecting(ref mut future) => {
                    match future.poll() {
                        Err(_) => {
                            /**
                             * If remote server is down, avoiding to "dos" it,
                             * waiting a reasonable amount of time between retries
                             */
                            thread::sleep(time::Duration::from_millis(100)); //TODO:! make millis configurable
                            Some(RemoteConnectionState::NotConnected)
                        }
                        Ok(Async::NotReady) => {
                            return Ok(AsyncSink::NotReady(msg));
                        }
                        Ok(Async::Ready(stream)) => {
                            info!("Connection with remote server is successful");
                            let middleware_tx = StubbornSink::get_inner_sink(stream, &self.handle); //TODO:! try to use self.get_inner_sink()
                            Some(RemoteConnectionState::Connected(middleware_tx))
                        }
                    }
                }
                RemoteConnectionState::NotConnected => {
                    Some(RemoteConnectionState::Connecting(self.connection_attempt()))
                }
            };

            match next_status {
                Some(s) => self.status = s,
                None => {}
            }
        }
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        Ok(Async::Ready(()))
    }
}


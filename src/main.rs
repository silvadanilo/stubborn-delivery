#[macro_use]
extern crate log;
extern crate env_logger;

extern crate futures;
extern crate tokio_core;
extern crate tokio_line;
extern crate getopts;
extern crate tokio_timer;

mod stubborn_sink;
mod server;

use futures::future::Future;
use futures::sync::mpsc;
use futures::Stream;
use getopts::Options;
use std::env;
use stubborn_sink::StubbornSink;
use std::io::{self, ErrorKind};
use server::Server;
use tokio_core::reactor::Core;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} FILE [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    env_logger::init().unwrap();

    let configuration = match handle_options() {
        Some(config) => config,
        None => return,
    };

    /**
     * every connected client receives a clone of buftx where to sends data
     * all received data are read from bufrx and sent the StubbornSink which will try to
     * sent it to the final destination. If the final destination is unreachable StubbornSink will
     * returns a NotReady error and received data will remains in this channel
     */
    let (buftx, bufrx) = mpsc::unbounded();

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stubborn_sink = StubbornSink::new(configuration.connect_to.parse().unwrap(),
                                          handle.clone());

    /**
     * sends all data received by clients to the remote server
     */
    let forwarding = bufrx
        .map_err(|_| io::Error::new(ErrorKind::Other, "should never happens"))
        .forward(stubborn_sink);
    handle.spawn(forwarding.map(|_| ()).map_err(|_| ()));

    /**
     * listening for client connections
     */
    let server = Server::new(configuration.clone(), handle.clone(), buftx.clone());
    let listening = server.accept_connection();

    core.run(listening).unwrap();
}

#[derive(Clone)]
pub struct Conf {
    listen_on: String,
    connect_to: String,
}

fn handle_options() -> Option<Conf> {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("l", "listen", "port on where listening", "PORT");
    opts.optopt("d", "destination", "remote address on where sends data", "ADDRESS:PORT");
    opts.optflag("h", "help", "print this help menu");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("{}\n", f);
            print_usage(&program, opts);
            return None;
        }
    };
    if matches.opt_present("h") {
        print_usage(&program, opts);
        return None;
    }

    let listen_on = matches.opt_str("l");
    let connect_to = matches.opt_str("d");

    if listen_on.is_none() || connect_to.is_none() {
        print_usage(&program, opts);
        return None;
    }

    Some(Conf {
        listen_on: listen_on.unwrap(),
        connect_to: connect_to.unwrap(),
    })
}

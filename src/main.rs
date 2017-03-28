#[macro_use] extern crate log;
extern crate env_logger;

extern crate futures;
extern crate tokio_core;
extern crate tokio_line;
extern crate getopts;

mod stubborn_sink;

use futures::future::Future;
use futures::sync::mpsc::{self};
use futures::{Stream, Sink};
use getopts::Options;
use std::env;
use std::io::{self, ErrorKind};
use stubborn_sink::StubbornSink;
use tokio_core::io::{Io};
use tokio_core::net::{TcpListener};
use tokio_core::reactor::{Core};
use tokio_line::LineCodec;

fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    /**
     * every connected client receives a clone of buftx where to sends data
     * all received data are read from bufrx and sent the StubbornSink which will try to
     * sent it to the final destination. If the final destination is unreachable StubbornSink will
     * returns a NotReady error and received data will remains in this channel
     */
    let (buftx, bufrx) = mpsc::unbounded();



    let address = "0.0.0.0:12345".parse().unwrap();
    let listener = TcpListener::bind(&address, &core.handle()).unwrap();
    let connections = listener.incoming();

    let server = connections.for_each(|(socket, _)| {
        let transport = socket.framed(LineCodec);

        let nonhocapitoperchedevoclonarlo = buftx.clone();
        let process_connection = transport.for_each(move |line| {
            nonhocapitoperchedevoclonarlo.clone().send(line)
                .map_err(|err| io::Error::new(ErrorKind::Other, err))
                .map(|_| ())
        });

        handle.spawn(process_connection.map_err(|_| ()));

        Ok(())
    });





    let stubborn_sink = StubbornSink::new(handle.clone());
    let f =
        bufrx.fold(stubborn_sink,
                   |stubborn_sink, message| stubborn_sink.send(message).map_err(|_| ()));

    handle.spawn(f.map(|_| ()).map_err(|_| ()));

    core.run(server).unwrap();
}

// fn simulated_messaging_receiving_from_clients(buftx: UnboundedSender<String>,
//                                               handle: &Handle)
//                                               -> () {

//     let timer = Timer::default();
//     let wakeups = timer.interval(Duration::new(0, 150000000));
//     let mut i = 0;
//     let background_tasks = wakeups.for_each(move |_| {
//         debug!("Interval");
//         i = i + 1;
//         buftx.clone()
//             // .send(format!("Messagio {}", i).to_string())
//             .send(format!(r#"{{"@timestamp":"2017-03-24T09:16:42.636040+01:00","@source":"dev-all-onebiptrusty cli","@fields":{{"channel":"integrationtest-client","level":100,"extra_level_name":"DEBUG","extra_uname":"dev-all-onebiptrusty","extra_sapi":"cli","extra_process_id":17954}},"@message":"Message {}"}}"#, i).to_string())
//             .map(|_| ())
//             .map_err(|_| TimerError::NoCapacity)
//     });

//     // let background_tasks = buftx.clone()
//     //     .send(format!("Messagio {}", i).to_string())
//     //     .map(|_| ())
//     //     .map_err(|_| ());

//     handle.spawn(background_tasks.map(|_| ()).map_err(|_| ()));
// }

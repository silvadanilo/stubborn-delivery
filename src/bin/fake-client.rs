use std::env;
use std::{thread, time};
use std::io::Write;
use std::net::TcpStream;
use std::fs::OpenOptions;

fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:12345").expect("StubbornSink seems down");

    for argument in env::args().skip(1) {
        let v = argument.splitn(2, ':').collect::<Vec<_>>();
        let action: (&str, &str) = (v[0], v[1]);
        match action {
            (key, value) => {
                match key {
                    "sleep" => {
                        thread::sleep(time::Duration::from_millis(value.parse::<u64>().unwrap()));
                    },
                    "send" => {
                        let _ = stream.write((value.to_string() + "\n").as_bytes());

                        let mut file = OpenOptions::new().append(true).open("/tmp/fake-client.txt").unwrap();
                        file.write_all((value.to_string() + "\n").as_bytes());
                    }
                    x => panic!("command `{}` is not supported", x),
                }
            }
        }
    }
}

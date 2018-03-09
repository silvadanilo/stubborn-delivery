#[macro_use]
extern crate quickcheck;

use quickcheck::{Arbitrary, QuickCheck, StdGen, TestResult, quickcheck};
use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Child, Stdio};
use std::str;
use std::{thread, time};
use std::error::Error;

#[derive(Clone, Debug)]
enum Action {
    NewClient(Vec<ClientAction>),
    RestartRemoteServer(u64),
    Sleep(u64),
}

#[derive(Clone, Debug)]
enum ClientAction {
    SendData(String),
    Sleep(u64),
}

impl Arbitrary for ClientAction {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        let size = g.size();
        let size = g.gen_range(1, size);
        let send_data = ClientAction::SendData(g.gen_ascii_chars().take(size).collect());
        let sleep = ClientAction::Sleep(g.gen_range(1, 100));

        g.choose(&[
            send_data,
            sleep,
        ]).unwrap().clone()
    }
}

impl Arbitrary for Action {
    fn arbitrary<G: quickcheck::Gen>(g: &mut G) -> Self {
        let sleep = Action::Sleep(g.gen_range(1, 100));
        let new_client = Action::NewClient(Arbitrary::arbitrary(g));
        let restart_remote_server = Action::RestartRemoteServer(g.gen_range(1, 100));

        g.choose(&[
            new_client,
            restart_remote_server,
            sleep,
        ]).unwrap().clone()
    }
}

quickcheck! {
    fn test_xxx(actions: Vec<Action>, xs: (Vec<isize>, Vec<usize>)) -> bool {
        let mut acceptance = AcceptanceTest::new();
        println!("config is: {:?}", acceptance);
        println!("actions are: {:?}", actions);

        acceptance.start_stubborn_sink();
        thread::sleep(time::Duration::from_millis(300));

        let expected = collect_expected_messages(&actions);

        for action in actions.iter() {
            match *action {
                Action::NewClient(ref client_actions) => {
                    acceptance.start_client(client_actions);
                },
                Action::RestartRemoteServer(_) => acceptance.restart_remote_server(),
                Action::Sleep(millis) => thread::sleep(time::Duration::from_millis(millis)),
            }
        }

        acceptance.ensure_all_clients_have_been_properly_terminated();
        thread::sleep(time::Duration::from_millis(1000));
        acceptance.close();

        let mut collected_output = acceptance.collect_output();
        collected_output.sort();

        assert_eq!(expected, collected_output);

        true
    }
}


fn collect_expected_messages(actions: &Vec<Action>) -> Vec<String> {
    let mut messages = actions
        .iter()
        .filter(|action| {
            match action {
                &&Action::NewClient(_) => true,
                _ => false
            }
        })
        .map(|action| {
            match action {
                &Action::NewClient(ref client_actions) => Some(client_actions),
                _ => None
            }
        })
        .filter(|client_actions| client_actions.is_some())
        .flat_map(|client_actions| {
            client_actions
                .unwrap()
                .iter()
                .filter(|client_action| {
                    match client_action {
                        &&ClientAction::SendData(_) => true,
                        _ => false,
                    }
                })
                .map(|client_action| {
                    match client_action {
                        &ClientAction::SendData(ref message) => message.clone(),
                        _ => "cannot happens".to_string(),
                    }
                })
                .collect::<Vec<String>>()
        })
        .collect::<Vec<String>>();

    messages.sort();

    messages
}

#[derive(Debug)]
struct AcceptanceTest {
    root: PathBuf,
    bin: PathBuf,
    remote_server_bin: PathBuf,
    client_bin: PathBuf,
    stubborn_sink_process: Option<Child>,
    remote_server_process: Option<Child>,
    remote_server_output: Vec<String>,
    client_processes: Vec<Child>,
    // childs: HashMap<&'static str, Child>,
}

impl AcceptanceTest {
    pub fn new() -> Self {
        let mut root = env::current_exe().unwrap()
            .parent()
            .expect("executable's directory")
            .to_path_buf();

        if root.ends_with("deps") {
            root.pop();
        }

        let bin = root.join("stubborn-sink");
        let remote_server_bin = root.join("fake-remote-server");
        let client_bin = root.join("fake-client");

        AcceptanceTest {
            root: root,
            bin: bin,
            remote_server_bin: remote_server_bin,
            client_bin: client_bin,
            stubborn_sink_process: None,
            remote_server_process: None,
            remote_server_output: vec![],
            client_processes: vec![],
            // childs: vec![],
        }
    }

    pub fn close(&mut self) {
        if let Some(ref mut child) = self.remote_server_process {
            child.kill();
            let mut s = String::new();
            match child.stdout.take().expect("No stdout on child").read_to_string(&mut s) {
                Err(why) => panic!("couldn't read wc stdout: {}", why.description()),
                Ok(_) => {},
            }

            if s != "" {
                for x in s.trim().split("\n").collect::<Vec<&str>>() { // something better than for?
                    self.remote_server_output.push(x.to_string());
                }
            }
        }
        self.remote_server_process = None;

        if let Some(ref mut child) = self.stubborn_sink_process {
            child.kill().expect("command wasn't running");
        }
    }

    pub fn ensure_all_clients_have_been_properly_terminated(&mut self) {
        for mut client_process in self.client_processes.iter_mut() {
            match client_process.wait() {
                Err(status_code) => panic!("a client has not been properly termianted, it exits with {} status code", status_code),
                _ => {}
            }
        }
    }

    pub fn collect_output(&mut self) -> Vec<String> {
        if let Some(ref mut child) = self.remote_server_process {
            child.kill();
            // self.remote_server_process = None;
            let mut s = String::new();
            match child.stdout.take().expect("No stdout on child").read_to_string(&mut s) {
                Err(why) => panic!("couldn't read wc stdout: {}", why.description()),
                Ok(_) => {},
            }

            if s != "" {
                for x in s.trim().split("\n").collect::<Vec<&str>>() { // something better than for?
                    self.remote_server_output.push(x.to_string());
                }
            }
        }

        self.remote_server_process = None;

        self.remote_server_output.clone()
    }

    pub fn start_client(&mut self, client_actions: &Vec<ClientAction>) {

        let args = client_actions.iter().map(|client_action| {
            match *client_action {
                ClientAction::SendData(ref text) => format!("send:{}", text),
                ClientAction::Sleep(millis) => format!("sleep:{}", millis),
            }
        }).collect::<Vec<String>>();

        let mut cmd = process::Command::new(&self.client_bin);
        let process = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .args(args)
            .spawn()
            .unwrap();

        self.client_processes.push(process);
    }

    pub fn start_stubborn_sink(&mut self) {
        let mut cmd = process::Command::new(&self.bin);
        let child = cmd
            .arg("-l 0.0.0.0:12345")
            .arg("-d 127.0.0.1:8765")
            .spawn()
            .unwrap();

        self.stubborn_sink_process = Some(child);
    }

    pub fn restart_remote_server(&mut self) {
        if let Some(ref mut child) = self.remote_server_process {
            child.kill().expect("command wasn't running");
            let mut s = String::new();
            match child.stdout.take().expect("No stdout on child").read_to_string(&mut s) {
                Err(why) => panic!("couldn't read wc stdout: {}", why.description()),
                Ok(_) => {},
            }

            if s != "" {
                for x in s.trim().split("\n").collect::<Vec<&str>>() { // something better than for?
                    self.remote_server_output.push(x.to_string());
                }
            }
        }

        let mut cmd = process::Command::new(&self.remote_server_bin);
        let mut child = cmd.stdout(Stdio::piped())
            .spawn()
            .expect("Cannot spawn child");

        self.remote_server_process = Some(child);
    }
}

impl Drop for AcceptanceTest {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.stubborn_sink_process {
            child.kill().expect("command wasn't running");
        }
        if let Some(ref mut child) = self.remote_server_process {
            child.kill();
        }

        // println!("OUTPUT OF REMOTE SERVER:\n{}", self.remote_server_output);
        // for ref mut child in self.childs.iter_mut() {
        //     child.kill().expect("command wasn't running");
        // }
    }
}

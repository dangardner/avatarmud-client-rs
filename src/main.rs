//! Client for Avatar MUD (avatar.outland.org:3000)
//!
//! Line-based telnet client with GMCP support
//!
//! # Installation
//! ```sh
//! cargo install avatarmud-client
//! ```
//!
//! # Usage
//! ```sh
//! avatarmud-client
//! ```

use telnet::{Telnet, Action, Event as TelnetEvent, TelnetOption};
use std::{io, io::Write, time::Duration};
use std::net::ToSocketAddrs;
use std::os::unix::io::{RawFd, AsRawFd};
use std::thread::sleep;
use nonblock::NonBlockingReader;
use termios::*;

static TARGET_ADDR:&str = "avatar.outland.org:3000";
static BUFFER_SIZE:usize = 65536;
static CONNECT_TIMEOUT:u64 = 5;
static DELAY_MILLIS:u64 = 50;
const TELOPT_GMCP:u8 = 201;

fn set_echo(fd: RawFd, echo:bool) {
    let mut termios = Termios::from_fd(fd)
        .expect("Failed to tcgetattr");
    if echo {
        termios.c_lflag |= ECHO;
    } else {
        termios.c_lflag &= !ECHO;
    }
    tcsetattr(fd, TCSANOW, &termios)
        .expect("Failed to tcsetattr");
}

fn main() {
    let address = TARGET_ADDR.to_socket_addrs()
        .expect("Failed to resolve hostname")
        .next()
        .expect("Address iterator returned none");
    let mut telnet = Telnet::connect_timeout(&address, BUFFER_SIZE, Duration::from_secs(CONNECT_TIMEOUT))
        .expect("Connection failed");
    println!("Connected to {TARGET_ADDR}");
    let telopt_gmcp = TelnetOption::parse(TELOPT_GMCP);
    telnet.negotiate(&Action::Do, telopt_gmcp)
        .expect("Failed to negotiate TELOPT_GMCP");

    set_echo(io::stdin().as_raw_fd(), true);
    let mut noblock_stdin = NonBlockingReader::from_fd(io::stdin())
        .expect("Failed to open non-blocking stdin");
    let mut input_buffer = String::new();

    loop {
        /* read from stdin */
        let mut buf = String::new();
        noblock_stdin.read_available_to_string(&mut buf).unwrap();
        input_buffer.push_str(&buf);
        let parts:Vec<&str> = input_buffer.splitn(2, '\n').collect();
        if parts.len() > 1 {
            telnet.write(parts[0].as_bytes()).unwrap();
            telnet.write(b"\n").unwrap();
            input_buffer = parts[1].to_string();
        }
        /* read from socket */
        let telnet_event = telnet.read_nonblocking().expect("Read error");
        match telnet_event {
            TelnetEvent::Data(buffer) => {
                io::stdout().write(&buffer)
                    .expect("Failed to write to stdout");
                if buffer.last().unwrap() != &b'\r' {
                    io::stdout().flush()
                        .expect("Failed to flush");
                }
            },
            TelnetEvent::Error(err) => {
                println!("{}", err);
                break;
            },
            TelnetEvent::Negotiation(Action::Wont, TelnetOption::Echo) => {
                set_echo(io::stdin().as_raw_fd(), true);
            },
            TelnetEvent::Negotiation(Action::Will, TelnetOption::Echo) => {
                set_echo(io::stdin().as_raw_fd(), false);
            },
            TelnetEvent::Negotiation(Action::Will, TelnetOption::UnknownOption(TELOPT_GMCP)) => {
                telnet.negotiate(&Action::Do, telopt_gmcp)
                    .expect("Failed to negotiate TELOPT_GMCP");
                telnet.subnegotiate(telopt_gmcp, "Core.Hello { \"client\": \"avatarmud-client-rs\", \"version\": \"0.1.0\" }".as_bytes())
                    .expect("Failed to send Core.Hello");
                telnet.subnegotiate(telopt_gmcp, "Core.Supports.Set [ \"Core 1\",\"Char 1\",\"Room 1\",\"Comm 1\",\"IRE.Composer 1\" ]".as_bytes())
                    .expect("Failed to send Core.Supports.Set");
            },
            TelnetEvent::Subnegotiation(TelnetOption::UnknownOption(TELOPT_GMCP), gmcp_message) => {
                println!("GMCP message received: {}", std::str::from_utf8(&*gmcp_message).unwrap());
            },
            _ => {}
        }
        sleep(Duration::from_millis(DELAY_MILLIS));
    }
}

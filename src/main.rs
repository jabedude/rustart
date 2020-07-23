use log::{LevelFilter, info};
use libsystemd::{activation, daemon};

use mio::net::UnixDatagram;
use mio::{Events, Interest, Poll, Token};

//use std::os::unix::net::UnixDatagram;
use std::time::Duration;
use std::os::unix::io::FromRawFd;
use std::os::unix::io::IntoRawFd;

/// /run/systemd/journal/socket
const SDJLOG: Token = Token(0);
/// /dev/log (I think)
const DEVLOG: Token = Token(1);
/// /run/systemd/journal/syslog
const SYSLOG: Token = Token(2);
/// Audit, looks like netlink
const AUDLOG: Token = Token(3);

fn main() {
    simple_logging::log_to_file("/vagrant/pid_eins.log", LevelFilter::Info).unwrap();

    let mut poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(128);

    let mut fds = activation::receive_descriptors(false).expect("Error receiving descriptors");
    info!("Received fds: {:?}", fds);

    let raw_fd = fds.remove(1).into_raw_fd();
    info!("Creating from {}", raw_fd);
    let mut devlog_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
    poll.registry().register(&mut devlog_sock, DEVLOG, Interest::READABLE).unwrap();

    let raw_fd = fds.remove(2).into_raw_fd();
    info!("Creating from {}", raw_fd);
    let mut sdlog_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
    poll.registry().register(&mut sdlog_sock, SDJLOG, Interest::READABLE).unwrap();

    let raw_fd = fds.remove(0).into_raw_fd();
    info!("Creating from {}", raw_fd);
    let mut unk_sock = unsafe { UnixDatagram::from_raw_fd(raw_fd) };
    poll.registry().register(&mut unk_sock, AUDLOG, Interest::READABLE).unwrap();

    info!("Sending initial notify");
    daemon::notify(false, &[daemon::NotifyState::Ready]).unwrap();

    loop {
        poll.poll(&mut events, Some(Duration::from_secs(60))).unwrap();
        info!("Notifying watchdog");
        daemon::notify(false, &[daemon::NotifyState::Watchdog]).unwrap();

        for event in events.iter() {
            match event.token() {
                SDJLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on /run/systemd/journal/socket");
                    sdlog_sock.recv(&mut buf).expect("Error recieving");
                    match std::str::from_utf8(&buf[..]) {
                        Ok(s) => info!("Recieved {}", s),
                        Err(_) => continue,
                    };
                }
                DEVLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on /dev/log");
                    devlog_sock.recv(&mut buf).expect("Error recieving");
                    match std::str::from_utf8(&buf[..]) {
                        Ok(s) => info!("Recieved {}", s),
                        Err(_) => continue,
                    };
                }
                AUDLOG => {
                    let mut buf = [0u8; 1024];
                    info!("Got read event on unk fd");
                    unk_sock.recv(&mut buf).expect("Error recieving");
                    match std::str::from_utf8(&buf[..]) {
                        Ok(s) => info!("Recieved {}", s),
                        Err(_) => continue,
                    };
                }
                _ => {
                    panic!();
                }
            }
        }
    }
}

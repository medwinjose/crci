use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let t_start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let mut stream = match TcpStream::connect("127.0.0.1:8000") {
        Ok(s) => s,
        Err(_) => {
            let t_end = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            println!("{} {} {} connect_err 0", t_start, t_end, t_end - t_start);
            return;
        }
    };

    let local_port = stream.local_addr().unwrap().port();

    let t_send = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let payload = b"\x00\x00\x00\xFFMALFORMED_DATA";
    if stream.write_all(payload).is_err() {
        let t_end = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        println!(
            "{} {} {} write_err {}",
            t_send,
            t_end,
            t_end - t_send,
            local_port
        );
        return;
    }

    let mut buf = [0; 1024];
    let event;
    match stream.read(&mut buf) {
        Ok(0) => event = "EOF",
        Ok(_) => event = "DATA",
        Err(e) => {
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
            {
                event = "timeout";
            } else if e.kind() == std::io::ErrorKind::ConnectionReset {
                event = "RST";
            } else {
                event = "error";
            }
        }
    }
    let t_end = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    println!(
        "{} {} {} {} {}",
        t_send,
        t_end,
        t_end - t_send,
        event,
        local_port
    );
}

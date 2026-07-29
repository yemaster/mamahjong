use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::ExitCode;
use std::time::Duration;

const DEFAULT_ADDRESS: &str = "127.0.0.1:8080";
const ADDRESS_ENV: &str = "MAMAHJONG_HEALTHCHECK_ADDRESS";
const TIMEOUT: Duration = Duration::from_secs(2);
const REQUEST: &[u8] =
    b"GET /health/ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";

fn main() -> ExitCode {
    match check() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("readiness endpoint returned a non-success status");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("health check failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn check() -> io::Result<bool> {
    let address = env::var(ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned());
    let address: SocketAddr = address.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid {ADDRESS_ENV}: {error}"),
        )
    })?;

    let mut stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    stream.write_all(REQUEST)?;

    let mut status_line = String::with_capacity(32);
    BufReader::new(stream).read_line(&mut status_line)?;

    Ok(is_ready_status(&status_line))
}

fn is_ready_status(status_line: &str) -> bool {
    status_line.starts_with("HTTP/1.1 200 ") || status_line.starts_with("HTTP/1.0 200 ")
}

#[cfg(test)]
mod tests {
    use super::is_ready_status;

    #[test]
    fn accepts_success_status() {
        assert!(is_ready_status("HTTP/1.1 200 OK\r\n"));
    }

    #[test]
    fn rejects_non_success_status() {
        assert!(!is_ready_status("HTTP/1.1 503 Service Unavailable\r\n"));
    }
}

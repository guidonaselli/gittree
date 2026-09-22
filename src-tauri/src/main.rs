#[cfg(unix)]
fn run_askpass_client(socket_path: &str, prompt: &str) -> bool {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let Ok(mut stream) = UnixStream::connect(socket_path) else {
        return false;
    };

    let single_line_prompt = prompt.replace(['\r', '\n'], " ");
    if writeln!(stream, "{}", single_line_prompt).is_err() {
        return false;
    }
    let _ = stream.flush();

    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }

    if response.is_empty() {
        return false;
    }

    print!("{}", response.trim_end_matches(&['\r', '\n'][..]));
    let _ = std::io::stdout().flush();
    true
}

fn main() {
    #[cfg(unix)]
    if let Ok(socket_path) = std::env::var("GITTREE_ASKPASS_SOCKET") {
        let prompt = std::env::args().nth(1).unwrap_or_default();
        if run_askpass_client(&socket_path, &prompt) {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }

    gittree_lib::run();
}

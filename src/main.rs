use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;
use std::usize;

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
    body: String,
}

impl HttpRequest {
    fn from_raw(request: &str) -> Self {
        let mut lines = request.lines();
        let mut headers = HashMap::new();
        let mut body = String::new();

        let request_line = lines.next().unwrap_or_default();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let version = parts.next().unwrap_or("").to_string();

        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }
            if let Some((key, value)) = line.split_once(": ") {
                headers.insert(key.to_string(), value.to_string());
            }
        }

        body = lines.collect::<Vec<&str>>().join("\n");

        HttpRequest {
            method,
            path,
            version,
            headers,
            body,
        }
    }
}

#[derive(Debug)]
struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Job>,
}

impl ThreadPool {
    fn new(size: usize) -> ThreadPool {
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            workers.push(Worker::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    fn execute<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender.send(Box::new(job)).unwrap();
    }
}

#[derive(Debug)]
struct Worker {
    _id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let job = receiver.lock().unwrap().recv();
            match job {
                Ok(job) => {
                    println!("Worker {id} executing job ...");
                    job();
                }
                Err(_) => break,
            }
        });

        Worker {
            _id: id,
            thread: Some(thread),
        }
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:3000").expect("Failed to bind port");
    let num_threads = num_cpus::get();
    let pool = ThreadPool::new(num_threads);

    println!("Server is runing on http://127.0.0.1:3000 with {num_threads} threads");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                pool.execute(|| handle_client(stream));
            }
            Err(e) => {
                println!("Connection failed: {}", e);
            }
        }
    }
}

fn handle_request(request: &HttpRequest) -> String {
    match request.path.as_str() {
        "/" => format_response("200 Ok", "Welcome to the Home Page!"),
        "/about" => format_response("200 Ok", "About Us Page"),
        "/hello" => format_response("200 Ok", "Hello, Rustacean!"),
        _ => format_response("404 Not Found", "Page Not Found"),
    }
}

fn format_response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length:{}\r\n\r\n{body}",
        body.len()
    )
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    if let Ok(bytes_read) = stream.read(&mut buffer) {
        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
        let request = HttpRequest::from_raw(&request_str);

        // Define a simple HTTP Response
        let response = handle_request(&request);
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
    }
}

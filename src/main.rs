use actix_web::{web, App, HttpResponse, HttpServer, Responder, post, get};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
struct Client {
    location: String,
    room: String,
    #[serde(skip_serializing)]
    stream: Arc<Mutex<TcpStream>>,
    #[serde(skip_serializing)]
    last_heartbeat: Instant,
}

type ClientList = Arc<Mutex<HashMap<(String, String), Client>>>;

#[derive(Deserialize)]
struct LaundryRequest {
    location: String,
    room: String,
    machine: String,
}

#[post("/")]
async fn laundry_handler(data: web::Json<LaundryRequest>, clients: web::Data<ClientList>) -> impl Responder {
    let mut clients_guard = clients.lock().unwrap();
    let key = (data.location.clone(), data.room.clone());

    if let Some(client) = clients_guard.get_mut(&key) {
        let message = format!("Machine: {}", data.machine);
        if let Err(e) = client.stream.lock().unwrap().write_all(message.as_bytes()) {
            return HttpResponse::InternalServerError().body(format!("Failed to send message: {}", e));
        }
        client.last_heartbeat = Instant::now();
        HttpResponse::Ok().body("Message sent to the client")
    } else {
        HttpResponse::NotFound().body("Client not found")
    }
}

#[get("/clients")]
async fn list_clients(clients: web::Data<ClientList>) -> impl Responder {
    let clients_guard = clients.lock().unwrap();
    let client_list: Vec<&Client> = clients_guard.values().collect();
    HttpResponse::Ok().json(client_list)
}

fn handle_client(mut stream: TcpStream, clients: ClientList) {
    let mut buffer = [0; 512];
    
    if let Ok(size) = stream.read(&mut buffer) {
        if size > 0 {
            let config: serde_json::Value = serde_json::from_slice(&buffer[..size]).unwrap_or_else(|_| {
                eprintln!("Failed to parse client config, ignoring this client.");
                return serde_json::json!({});
            });

            let location = config["location"].as_str().unwrap_or("").to_string();
            let room = config["room"].as_str().unwrap_or("").to_string();

            if location.is_empty() || room.is_empty() {
                eprintln!("Invalid client configuration. Ignoring this client.");
                return;
            }

            println!("Client connected: Location: {}, Room: {}", location, room);

            let client = Client {
                location: location.clone(),
                room: room.clone(),
                stream: Arc::new(Mutex::new(stream.try_clone().unwrap())),
                last_heartbeat: Instant::now(),
            };
            clients.lock().unwrap().insert((location.clone(), room.clone()), client);

            let clients_clone = Arc::clone(&clients);
            thread::spawn(move || {
                handle_client_messages(stream, &location, &room, clients_clone);
            });
        }
    } else {
        eprintln!("Error reading from stream, client ignored.");
    }
}

fn handle_client_messages(mut stream: TcpStream, location: &str, room: &str, clients: ClientList) {
    let mut buffer = [0; 512];
    loop {
        match stream.read(&mut buffer) {
            Ok(size) if size > 0 => {
                let message = String::from_utf8_lossy(&buffer[..size]);
                if message.trim() == "KEEP_ALIVE" {
                    if let Some(client) = clients.lock().unwrap().get_mut(&(location.to_string(), room.to_string())) {
                        client.last_heartbeat = Instant::now();
                    }
                }
            },
            Ok(_) => break, // Connection closed
            Err(_) => break, // Error occurred
        }
    }
    println!("Client disconnected: Location: {}, Room: {}", location, room);
    clients.lock().unwrap().remove(&(location.to_string(), room.to_string()));
}

fn remove_inactive_clients(clients: &ClientList) {
    let mut clients_guard = clients.lock().unwrap();
    clients_guard.retain(|_, client| {
        if client.last_heartbeat.elapsed() > Duration::from_secs(60) {
            println!("Removing inactive client: Location: {}, Room: {}", client.location, client.room);
            false
        } else {
            true
        }
    });
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let clients: ClientList = Arc::new(Mutex::new(HashMap::new()));

    let clients_clone = Arc::clone(&clients);
    thread::spawn(move || {
        let listener = TcpListener::bind("0.0.0.0:25651").expect("Failed to bind to port 25651");
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let clients_clone = Arc::clone(&clients_clone);
                    thread::spawn(move || handle_client(stream, clients_clone));
                }
                Err(e) => {
                    eprintln!("Connection failed: {}. Continuing to accept new clients...", e);
                }
            }
        }
    });

    let clients_clone = Arc::clone(&clients);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(30));
            remove_inactive_clients(&clients_clone);
        }
    });

    HttpServer::new(move || {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600)
            )
            .app_data(web::Data::new(Arc::clone(&clients)))
            .service(laundry_handler)
            .service(list_clients)
    })
    .bind("0.0.0.0:25652")?
    .run()
    .await
}

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

type SplitSocket = (
    SplitSink<WebSocketStream<TcpStream>, Message>,
    SplitStream<WebSocketStream<TcpStream>>,
);

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    client_manager: Arc<Mutex<ClientManager>>,
) {
    match accept_async(stream).await {
        Ok(mut socket) => {
            let ip = addr.ip().to_string();
            let mut locked_client_manager = client_manager.lock_owned().await;

            let connections = locked_client_manager
                .clients
                .values()
                .filter(|client| client.address == ip)
                .count();

            if connections > 2 {
                println!("Socket from {} terminated due to too many connections", ip);
                socket.close(None).await.unwrap();
                return;
            }

            let client = locked_client_manager.create_client(socket.split(), ip.clone());

            println!("Socket {} from {} connected", client.index, ip);

            client.talk(&format!("Hello {}!", client.index)).await;
            client.hear().await;
        }
        Err(e) => println!("Error: {}", e),
    }
}

#[tokio::main]
async fn main() {
    let client_manager = Arc::new(Mutex::new(ClientManager::new()));

    let listener = TcpListener::bind("localhost:3000").await.unwrap();

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                handle_connection(stream, addr, Arc::clone(&client_manager)).await;
            }
            Err(e) => println!("Error: {}", e),
        }
    }
}

struct Client {
    index: usize,
    address: String,
    socket: SplitSocket,
}

impl Client {
    fn new(index: usize, socket: SplitSocket, address: String) -> Client {
        Client {
            index,
            socket,
            address,
        }
    }

    async fn talk(&mut self, message: &str) {
        if let Err(e) = self.socket.0.send(Message::text(message)).await {
            eprintln!("Error sending message: {}", e);
        }
    }

    async fn hear(&mut self) {
        if let Some(Ok(message)) = self.socket.1.next().await {
            println!("{} from {}: {:?}", self.index, self.address, message);
        }
    }
}

struct ClientManager {
    counter: usize,
    clients: HashMap<usize, Client>,
}

impl ClientManager {
    fn new() -> ClientManager {
        ClientManager {
            counter: 0,
            clients: HashMap::new(),
        }
    }

    fn create_client(&mut self, socket: SplitSocket, address: String) -> &mut Client {
        let index = self.counter;
        self.counter += 1;
        self.clients
            .entry(index)
            .or_insert_with(|| Client::new(index, socket, address))
    }

    #[allow(dead_code)]
    fn destroy_client(&mut self, index: usize) {
        self.clients.remove(&index);
    }

    #[allow(dead_code)]
    fn get_client(&mut self, index: usize) -> Option<&mut Client> {
        self.clients.get_mut(&index)
    }
}

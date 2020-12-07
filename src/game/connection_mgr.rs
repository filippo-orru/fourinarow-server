use crate::game::msg::ServerMessage;
use actix::*;

use super::{
    client_state::{ClientState, ClientStateMessage},
    lobby_mgr::LobbyManager,
};

const SEND_SERVER_INFO_INTERVAL_SECONDS: u64 = 2;

enum BacklinkState {
    Linked(Addr<LobbyManager>),
    Unlinked,
}

pub struct ConnectionManager {
    lobby_mgr_state: BacklinkState,
    connections: Vec<Connection>,
    player_in_queue: bool,
    send_server_info_batched: bool,
}

impl ConnectionManager {
    pub fn new() -> Self {
        ConnectionManager {
            lobby_mgr_state: BacklinkState::Unlinked,
            connections: Vec::new(),
            player_in_queue: false,
            send_server_info_batched: false,
        }
    }

    fn send_server_info_to_all(&self, ctx: &mut Context<Self>) {
        for connection in self.connections.iter() {
            self.send_server_info(connection.state_addr.clone(), ctx);
        }
    }

    fn send_server_info(&self, client_state_addr: Addr<ClientState>, _ctx: &mut Context<Self>) {
        // Return info about current server state
        let number_of_connections = self.connections.len();
        client_state_addr.do_send(ClientStateMessage::CurrentServerState(
            number_of_connections,
            self.player_in_queue,
            false,
        ));
    }
}

#[derive(Clone)]
struct Connection {
    state_addr: Addr<ClientState>,
}

pub enum ConnectionManagerMsg {
    Hello(Addr<ClientState>),               // sent when client first connects
    Bye(Addr<ClientState>),                 // sent when client disconnects
    Update(bool), // (player_in_queue): sent by lobbyManager when clients should be notified
    ChatMessage(Addr<ClientState>, String), // global chat message (sender_addr, msg)
    Backlink(Addr<LobbyManager>), // sent by lobbyManager when it starts to form bidirectional link
}

impl Message for ConnectionManagerMsg {
    type Result = Result<(), ()>;
}

impl Handler<ConnectionManagerMsg> for ConnectionManager {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ConnectionManagerMsg, _ctx: &mut Self::Context) -> Self::Result {
        use ConnectionManagerMsg::*;
        match msg {
            Hello(client_state_addr) => {
                // Add this new connection to list
                self.connections.push(Connection {
                    state_addr: client_state_addr.clone(),
                });

                // <- Commented out for performance reasons ->
                // self.send_server_info_to_all(ctx);
                self.send_server_info_batched = true;
            }
            Bye(client_state_addr) => {
                // Remove this connection from list if exists
                if let Some(index) = self
                    .connections
                    .iter()
                    .position(|conn| conn.state_addr == client_state_addr)
                {
                    self.connections.remove(index);
                    self.send_server_info_batched = true;
                }
            }
            Update(player_in_lobby) => {
                self.player_in_queue = player_in_lobby;
                self.send_server_info_batched = true;
            }
            ChatMessage(client_state_addr, msg) => {
                for connection in self.connections.iter() {
                    if connection.state_addr != client_state_addr {
                        connection
                            .state_addr
                            .do_send(ServerMessage::ChatMessage(true, msg.clone()));
                    }
                }
            }
            Backlink(lobby_mgr_addr) => {
                self.lobby_mgr_state = BacklinkState::Linked(lobby_mgr_addr)
            }
        }
        Ok(())
    }
}

impl Actor for ConnectionManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // let command = if cfg!(target_os = "macos") {
        //     "top -l 1 -stats \"cpu, command\" | grep fourinarow | awk '{print $1}'"
        // } else if cfg!(target_os = "linux") {
        //     "top -b -n 1 -d 0.2 | grep fourinarow | awk '{print $9}'"
        // } else {
        //     return;
        // };
        // ctx.run_interval(clock::Duration::from_secs(4), move |_, _| {
        //     let output = std::process::Command::new(command)
        //         .output()
        //         .map(|o| String::from_utf8(o.stdout))
        //         .expect("failed to run top command")
        //         .expect("failed to get top command output")
        //         .parse::<f32>()
        //         .expect("failed to parse top cpu usage");

        //     println!("Top output: {:?}", output);
        // });

        // Send currentserverinfo to everyone every x seconds (only if change occurred)
        ctx.run_interval(
            std::time::Duration::from_secs(SEND_SERVER_INFO_INTERVAL_SECONDS),
            |act, ctx| {
                if act.send_server_info_batched {
                    act.send_server_info_to_all(ctx);
                    act.send_server_info_batched = false;
                }
            },
        );
    }
}

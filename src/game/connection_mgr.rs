use crate::game::msg::ServerMessage;
use actix::*;

use super::{
    client_state::{ClientState, ClientStateMessage},
    lobby_mgr::{GetIsPlayerWaitingMsg, LobbyManager},
};

const SEND_SERVER_INFO_INTERVAL_SECONDS: u64 = 4;

pub struct ConnectionManager {
    lobby_mgr: Addr<LobbyManager>,
    connections: Vec<Connection>,
}

impl ConnectionManager {
    pub fn new(lobby_mgr: Addr<LobbyManager>) -> Self {
        ConnectionManager {
            lobby_mgr,
            connections: Vec::new(),
        }
    }

    fn send_server_info_to_all(&self, ctx: &mut Context<Self>) {
        for connection in self.connections.iter() {
            self.send_server_info(connection.state_addr.clone(), ctx);
        }
    }

    fn send_server_info(&self, client_state_addr: Addr<ClientState>, ctx: &mut Context<Self>) {
        // Return info about current server state
        let number_of_connections = self.connections.len();
        self.lobby_mgr
            .send(GetIsPlayerWaitingMsg)
            .into_actor(self)
            .then(
                move |player_waiting_result: Result<bool, MailboxError>, _, _| {
                    client_state_addr
                        .do_send(ClientStateMessage::CurrentServerState(
                            number_of_connections,
                            player_waiting_result.unwrap_or(false),
                            false,
                        ));
                    fut::ready(())
                },
            )
            .wait(ctx);
    }
}

#[derive(Clone)]
struct Connection {
    state_addr: Addr<ClientState>
}

pub enum ConnectionManagerMsg {
    Hello(Addr<ClientState>), // sent when client first connects
    Bye(Addr<ClientState>),   // sent when client disconnects
    ChatMessage(Addr<ClientState>, String), // global chat message (sender_addr, msg)
}

impl Message for ConnectionManagerMsg {
    type Result = Result<(), ()>;
}

impl Handler<ConnectionManagerMsg> for ConnectionManager {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ConnectionManagerMsg, ctx: &mut Self::Context) -> Self::Result {
        use ConnectionManagerMsg::*;
        match msg {
            Hello(client_state_addr) => {
                // Add this new connection to list
                self.connections.push(Connection {
                    state_addr: client_state_addr.clone()
                });

                // Send to everyone (including newly joined)
                self.send_server_info_to_all(ctx);

                // But also send every x seconds because player_is_waiting is not reactive (or in case one message gets lost)
                ctx.run_interval(
                    std::time::Duration::from_secs(SEND_SERVER_INFO_INTERVAL_SECONDS),
                    move |act, ctx| {
                        act.send_server_info(client_state_addr.clone(), ctx);
                    },
                );
            }
            Bye(client_state_addr) => {
                // Remove this connection from list if exists
                if let Some(index) = self
                    .connections
                    .iter()
                    .position(|conn| conn.state_addr == client_state_addr)
                {
                    self.connections.remove(index);
                    self.send_server_info_to_all(ctx);
                }
            }
            ChatMessage(client_state_addr, msg) => {
                for connection in self.connections.iter() {
                    if connection.state_addr != client_state_addr {
                    connection.state_addr.do_send(ServerMessage::ChatMessage(true, msg.clone()));
                    }
                }
            }
        }
        Ok(())
    }
}

impl Actor for ConnectionManager {
    type Context = Context<Self>;
}

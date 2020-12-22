use actix::*;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::{collections::HashMap, time::Instant};

use super::{
    client_adapter::{ClientAdapter, ClientAdapterMsg, ClientMsgString},
    client_connection::{ClientConnnectionMsg, ConnectionType},
    client_state::{ClientState, ClientStateMessage},
    lobby_mgr::LobbyManager,
    ClientConnection,
};
use crate::{api::users::user_mgr::UserManager, game::msg::ServerMessage};

pub type SessionToken = String;

const SEND_SERVER_INFO_INTERVAL_SECONDS: u64 = 2;

const CONNECTION_KEEPALIVE_SECONDS: u64 = 30;

enum BacklinkState {
    Linked(Addr<LobbyManager>),
    Unlinked,
}

pub struct ConnectionManager {
    lobby_mgr_state: BacklinkState,
    connections: HashMap<SessionToken, Connection>,
    player_in_queue: bool,
    send_server_info_batched: bool,
}

impl ConnectionManager {
    pub fn new() -> Self {
        ConnectionManager {
            lobby_mgr_state: BacklinkState::Unlinked,
            connections: HashMap::new(),
            player_in_queue: false,
            send_server_info_batched: false,
        }
    }

    fn send_server_info_interval(&self, ctx: &mut Context<Self>) {
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

    fn check_connectionstate_interval(&self, ctx: &mut Context<Self>) {
        ctx.run_interval(std::time::Duration::from_secs(1), |act, _| {
            let connections_to_remove = act
                .connections
                .iter()
                .filter_map(|(id, connection)| {
                    if let ConnectionState::Disconnected(disconnect_instant) = connection.state {
                        if disconnect_instant.elapsed().as_secs() >= CONNECTION_KEEPALIVE_SECONDS {
                            return Some((id.clone(), connection.clone()));
                        }
                    }
                    None
                })
                .collect::<Vec<(String, Connection)>>();

            for (id, connection) in connections_to_remove {
                connection.adapter_addr.do_send(ClientAdapterMsg::Close);
                act.connections.remove(&id);
                println!("Connection {} timeouted", id);
                act.send_server_info_batched = true;
            }
        });
    }

    fn send_server_info_to_all(&self, ctx: &mut Context<Self>) {
        for (_, connection) in self.connections.iter() {
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

    fn generate_session_token() -> SessionToken {
        thread_rng().sample_iter(&Alphanumeric).take(32).collect()
    }
}

#[derive(Clone)]
enum ConnectionState {
    Connected,
    Disconnected(Instant),
}

#[derive(Clone)]
struct Connection {
    adapter_addr: Addr<ClientAdapter>,
    state_addr: Addr<ClientState>,
    state: ConnectionState,
}

pub enum ConnectionManagerMsg {
    Disconnect {
        session_token: SessionToken,
        is_legacy: bool,
    },
    Update(bool), // (player_in_queue): sent by lobbyManager when clients should be notified
    ChatMessage(SessionToken, String), // global chat message (sender_addr, msg)
    ChatRead(SessionToken),
    Backlink(Addr<LobbyManager>), // sent by lobbyManager when it starts to form bidirectional link
    RequestAdapterNew(NewAdapterAdresses), // sent when client first connects
    RequestAdapterExisting(NewAdapterAdresses, String), // sent when client reconnects
    RequestAdapterLegacy(NewAdapterAdresses, String), // sent when legacy client first connects with playerMsgStr in "queue"
}

pub struct NewAdapterAdresses {
    pub client_conn: Addr<ClientConnection>,
    pub lobby_mgr: Addr<LobbyManager>,
    pub user_mgr: Addr<UserManager>,
}

impl Message for ConnectionManagerMsg {
    type Result = Result<(), ()>;
}

impl Handler<ConnectionManagerMsg> for ConnectionManager {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ConnectionManagerMsg, ctx: &mut Self::Context) -> Self::Result {
        use ConnectionManagerMsg::*;
        match msg {
            // Close(id) => {
            //     if let Some(connection) = self.connections.remove(id) {
            //         connection.adapter_addr.do_send(ClientAdapterMsg::Close)
            //     }
            // }
            Disconnect {
                session_token,
                is_legacy,
            } => {
                if is_legacy {
                    // Remove this connection from list if exists
                    if let Some(connection) = self.connections.get(&session_token).cloned() {
                        connection.adapter_addr.do_send(ClientAdapterMsg::Close);
                        self.connections.remove_entry(&session_token);
                    }
                } else {
                    if let Some(connection) = self.connections.get_mut(&session_token) {
                        // Set disconnected
                        connection
                            .adapter_addr
                            .do_send(ClientAdapterMsg::Disconnect);
                        connection.state = ConnectionState::Disconnected(Instant::now());
                    }
                }
                self.send_server_info_batched = true;
            }
            Update(player_in_lobby) => {
                self.player_in_queue = player_in_lobby;
                self.send_server_info_batched = true;
            }
            ChatMessage(sender_id, msg) => {
                for (id, connection) in self.connections.iter() {
                    if id != &sender_id {
                        connection.state_addr.do_send(ServerMessage::ChatMessage(
                            true,
                            msg.clone(),
                            Some(sender_id[0..5].into()),
                        ));
                    }
                }
            }
            ChatRead(sender_id) => {
                for (id, connection) in self.connections.iter() {
                    if id != &sender_id {
                        connection.state_addr.do_send(ServerMessage::ChatRead(true));
                    }
                }
            }
            Backlink(lobby_mgr_addr) => {
                self.lobby_mgr_state = BacklinkState::Linked(lobby_mgr_addr)
            }
            RequestAdapterNew(new_adapter_addresses) => {
                let session_token = Self::generate_session_token();
                let client_state_addr = ClientState::new(
                    session_token.clone(),
                    new_adapter_addresses.lobby_mgr,
                    new_adapter_addresses.user_mgr,
                    ctx.address(),
                )
                .start();
                let client_adapter = ClientAdapter::new(
                    new_adapter_addresses.client_conn.clone(),
                    client_state_addr.clone(),
                )
                .start();
                // Add this new connection to list
                self.connections.insert(
                    session_token.clone(),
                    Connection {
                        state_addr: client_state_addr.clone(),
                        adapter_addr: client_adapter.clone(),
                        state: ConnectionState::Connected,
                    },
                );
                new_adapter_addresses
                    .client_conn
                    .do_send(ClientConnnectionMsg {
                        session_token,
                        client_adapter,
                        connection_type: ConnectionType::Reliable { is_new: true },
                    });

                // <- Commented out for performance reasons ->
                // self.send_server_info_to_all(ctx);
                self.send_server_info_batched = true;
            }
            RequestAdapterExisting(new_adapter_addresses, session_token) => {
                if let Some(connection) = self.connections.get_mut(&session_token) {
                    connection.state = ConnectionState::Connected;
                    connection.adapter_addr.do_send(ClientAdapterMsg::Connect(
                        new_adapter_addresses.client_conn.clone(),
                    ));
                    new_adapter_addresses
                        .client_conn
                        .do_send(ClientConnnectionMsg {
                            session_token,
                            client_adapter: connection.adapter_addr.clone(),
                            connection_type: ConnectionType::Reliable { is_new: false },
                        });
                } else {
                    ctx.notify(RequestAdapterNew(new_adapter_addresses));
                }
            }
            RequestAdapterLegacy(new_adapter_addresses, str_msg) => {
                let session_token = Self::generate_session_token();
                let client_state_addr = ClientState::new(
                    session_token.clone(),
                    new_adapter_addresses.lobby_mgr,
                    new_adapter_addresses.user_mgr,
                    ctx.address(),
                )
                .start();
                let client_adapter = ClientAdapter::legacy(
                    new_adapter_addresses.client_conn.clone(),
                    client_state_addr.clone(),
                )
                .start();
                // Add this new connection to list
                self.connections.insert(
                    session_token.clone(),
                    Connection {
                        state_addr: client_state_addr.clone(),
                        adapter_addr: client_adapter.clone(),
                        state: ConnectionState::Connected,
                    },
                );
                // Backlink
                new_adapter_addresses
                    .client_conn
                    .do_send(ClientConnnectionMsg {
                        session_token,
                        client_adapter: client_adapter.clone(),
                        connection_type: ConnectionType::Legacy,
                    });

                client_adapter.do_send(ClientMsgString(str_msg));
                self.send_server_info_batched = true;
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

        self.send_server_info_interval(ctx);
        self.check_connectionstate_interval(ctx);
    }
}

use super::client_conn::ClientConnection;
use super::game_info::*;
use super::lobby::*;
use super::lobby_mgr::{self, *};
use super::msg::*;

use actix::*;

pub struct ClientState {
    lobby_mgr: Addr<LobbyManager>,
    backlinked_state: BacklinkState,
    conn_state: ClientConnState,
}

pub enum ClientConnState {
    Idle,
    InLobby(Player, Addr<Lobby>),
}

impl ClientState {
    pub fn new(lobby_mgr: Addr<LobbyManager>) -> ClientState {
        ClientState {
            lobby_mgr,
            backlinked_state: BacklinkState::Waiting,
            conn_state: ClientConnState::Idle,
        }
    }
}

enum BacklinkState {
    Waiting,
    Linked(Addr<ClientConnection>),
}

pub enum ClientStateMessage {
    BackLink(Addr<ClientConnection>),
    Reset,
    Timeout, // Triggered by client timeout
}

impl Handler<ClientStateMessage> for ClientState {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: ClientStateMessage, ctx: &mut Self::Context) -> Self::Result {
        use ClientStateMessage::*;
        match msg {
            BackLink(addr) => {
                if let BacklinkState::Waiting = self.backlinked_state {
                    self.backlinked_state = BacklinkState::Linked(addr);
                }
            }
            Timeout => {
                println!("ClientState: Client timed out.");
                if let ClientConnState::InLobby(player, lobby_addr) = &self.conn_state {
                    lobby_addr.do_send(ClientLobbyMessageNamed {
                        sender: *player,
                        msg: ClientLobbyMessage::PlayerLeaving,
                    });
                    self.conn_state = ClientConnState::Idle;
                }
                ctx.stop();
            }
            Reset => {
                self.conn_state = ClientConnState::Idle;
            }
        }
        Ok(())
    }
}

impl Handler<PlayerMessage> for ClientState {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: PlayerMessage, ctx: &mut Self::Context) -> Self::Result {
        let ok = Ok(());
        let err = Err(());
        use PlayerMessage::*;

        if let BacklinkState::Linked(ref client_conn_addr) = self.backlinked_state {
            match msg {
                PlaceChip(column) => {
                    if let ClientConnState::InLobby(player, lobby_addr) = &self.conn_state {
                        lobby_addr.do_send(ClientLobbyMessageNamed {
                            sender: *player,
                            msg: ClientLobbyMessage::PlaceChip(column),
                        });
                        ok
                    } else {
                        client_conn_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::NotInLobby)));
                        err
                    }
                }
                PlayAgainRequest => {
                    if let ClientConnState::InLobby(player, lobby_addr) = &self.conn_state {
                        lobby_addr.do_send(ClientLobbyMessageNamed {
                            sender: *player,
                            msg: ClientLobbyMessage::PlayAgainRequest,
                        });
                        ok
                    } else {
                        client_conn_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::NotInLobby)));
                        err
                    }
                }
                Leaving => {
                    match &self.conn_state {
                        ClientConnState::InLobby(player, lobby_addr) => {
                            println!("ClientState: forwarding Leave message to lobby.");
                            if lobby_addr
                                .try_send(ClientLobbyMessageNamed {
                                    sender: *player,
                                    msg: ClientLobbyMessage::PlayerLeaving,
                                })
                                .is_err()
                            {
                                // client_conn_addr
                                //     .do_send(ServerMessage::Error(Some(SrvMsgError::Internal)));
                                /// TODO: Lobby is dead. Send okay or error here?
                            }
                        }
                        ClientConnState::Idle => {
                            client_conn_addr
                                .do_send(ServerMessage::Error(Some(SrvMsgError::NotInLobby)));
                        }
                    }
                    self.conn_state = ClientConnState::Idle;
                    // ctx.stop();
                    ok
                }
                LobbyRequest => {
                    if let ClientConnState::Idle = &self.conn_state {
                        let client_conn_addr = client_conn_addr.clone();
                        self.lobby_mgr
                            .send(lobby_mgr::LobbyRequest::NewLobby(client_conn_addr.clone()))
                            .into_actor(self)
                            .then(move |res, act, _ctx| {
                                if let Ok(lobbyreq_resp_res) = res {
                                    if let Ok(lobbyreq_resp) = lobbyreq_resp_res {
                                        act.conn_state = ClientConnState::InLobby(
                                            lobbyreq_resp.player,
                                            lobbyreq_resp.lobby_addr,
                                        );
                                    }
                                } else {
                                    client_conn_addr
                                        .do_send(ServerMessage::Error(Some(SrvMsgError::Internal)));
                                }
                                fut::ready(())
                            })
                            .wait(ctx);
                        ok
                    } else {
                        client_conn_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::GameAlreadyStarted)));
                        err
                    }
                }
                LobbyJoin(id) => {
                    if let ClientConnState::Idle = &self.conn_state {
                        self.lobby_mgr
                            .send(lobby_mgr::LobbyRequest::JoinLobby(
                                id,
                                client_conn_addr.clone(),
                            ))
                            .into_actor(self)
                            .then(|res, act, _ctx| {
                                if let Ok(lobbyreq_resp_res) = res {
                                    if let Ok(lobbyreq_resp) = lobbyreq_resp_res {
                                        act.conn_state = ClientConnState::InLobby(
                                            lobbyreq_resp.player,
                                            lobbyreq_resp.lobby_addr,
                                        );
                                    }
                                }
                                fut::ready(())
                            })
                            .wait(ctx);
                        ok
                    } else {
                        client_conn_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::GameAlreadyStarted)));
                        err
                    }
                }
            }
        } else {
            err
        }
    }
}

// impl Handler<ServerMessage> for ClientState {
//     type Result = Result<(),()>;

//     fn handle(&mut self, msg: ServerMessage, ctx: &mut Self::Context) -> Self::Result {
//         match msg {
//             ServerMessage::Reset
//         }
//     }
// }

impl Message for ClientStateMessage {
    type Result = Result<(), ()>;
}

impl Actor for ClientState {
    type Context = Context<Self>;
}

use std::time::Duration;

use super::lobby_mgr::{self, *};
use super::msg::*;
use super::{
    client_adapter::{ClientAdapter, ClientAdapterMsg},
    connection_mgr::ConnectionManager,
};
use super::{connection_mgr::ConnectionManagerMsg, game_info::*};
use super::{connection_mgr::WSSessionToken, lobby::*};
use crate::{
    api::users::{
        user::PublicUserMe,
        user_mgr::{self, UserManager},
    },
    logging::*,
};

use actix::*;

pub struct ClientState {
    id: WSSessionToken,
    lobby_mgr: Addr<LobbyManager>,
    user_mgr: Addr<UserManager>,
    _logger: Addr<Logger>,
    connection_mgr: Addr<ConnectionManager>,
    backlinked_state: BacklinkState,
    lobby_state: ClientLobbyState,
    maybe_user_info: Option<PublicUserMe>,
}

#[derive(Clone)]
pub enum ClientLobbyState {
    Idle,
    InLobbyWaitingForHost { player: Player, lobby: Addr<Lobby> },
    InLobby { player: Player, lobby: Addr<Lobby> },
}

impl ClientState {
    pub fn new(
        id: WSSessionToken,
        lobby_mgr: Addr<LobbyManager>,
        user_mgr: Addr<UserManager>,
        _logger: Addr<Logger>,
        connection_mgr: Addr<ConnectionManager>,
    ) -> ClientState {
        ClientState {
            id,
            lobby_mgr,
            user_mgr,
            _logger,
            connection_mgr,
            backlinked_state: BacklinkState::Waiting,
            lobby_state: ClientLobbyState::Idle,
            maybe_user_info: None,
        }
    }

    fn received_lobby_request_response(
        &mut self,
        res: Result<Result<LobbyRequestResponse, Option<SrvMsgError>>, MailboxError>,
        ctx: &mut <Self as Actor>::Context,
        client_adapter_addr: Addr<ClientAdapter>,
    ) {
        match res {
            Ok(lobby_request_response_result) => {
                match lobby_request_response_result {
                    Ok(lobby_request_response) => {
                        match lobby_request_response {
                            LobbyRequestResponse {
                                player,
                                lobby_addr,
                                waiting,
                                ..
                            } => {
                                if waiting {
                                    self.lobby_state = ClientLobbyState::InLobbyWaitingForHost {
                                        player,
                                        lobby: lobby_addr,
                                    };
                                    // In case other player does not respond
                                    ctx.run_later(Duration::from_millis(1000), move |act, _ctx| {
                                        if let ClientLobbyState::InLobbyWaitingForHost { .. } =
                                            act.lobby_state
                                        {
                                            client_adapter_addr.clone().do_send(
                                                ServerMessage::Error(Some(
                                                    SrvMsgError::LobbyNotFound,
                                                )),
                                            );
                                        }
                                    });
                                } else {
                                    client_adapter_addr.do_send(ServerMessage::Okay);
                                    self.lobby_state = ClientLobbyState::InLobby {
                                        player,
                                        lobby: lobby_addr,
                                    };
                                }
                            }
                        }
                    }
                    Err(maybe_server_err) => {
                        client_adapter_addr.do_send(ServerMessage::Error(maybe_server_err));
                    }
                }
            }
            Err(_) => {
                client_adapter_addr.do_send(ServerMessage::Error(Some(SrvMsgError::Internal)));
            }
        }
    }
}

enum BacklinkState {
    Waiting,
    Linked(Addr<ClientAdapter>),
}

pub enum ClientStateMessage {
    BackLink(Addr<ClientAdapter>),
    Reset,
    Close, // Triggered by client timeout or disconnect
    BattleReqJoinLobby(Addr<Lobby>),
    CurrentServerState(usize, bool, bool), // connected players, someone wants to play, [internal: was requeued]
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
            Close => {
                if let ClientLobbyState::InLobby {
                    player,
                    lobby: lobby_addr,
                } = &self.lobby_state
                {
                    lobby_addr.do_send(ClientLobbyMessageNamed {
                        sender: *player,
                        msg: ClientLobbyMessage::PlayerLeaving {
                            reason: PlayerLeaveReason::Disconnect,
                        },
                    });
                    self.lobby_state = ClientLobbyState::Idle;
                }
                ctx.stop();
            }
            Reset => {
                self.lobby_state = ClientLobbyState::Idle;
            }
            BattleReqJoinLobby(addr) => {
                if let BacklinkState::Linked(ref client_conn_addr) = self.backlinked_state {
                    self.lobby_state = ClientLobbyState::InLobby {
                        player: Player::One,
                        lobby: addr,
                    };
                    client_conn_addr.do_send(ServerMessage::Okay);
                }
            }
            CurrentServerState(connected_players, player_waiting, requequed) => {
                if let BacklinkState::Linked(ref client_conn_addr) = self.backlinked_state {
                    client_conn_addr.do_send(ServerMessage::CurrentServerState(
                        connected_players,
                        player_waiting,
                    ));
                } else if requequed {
                    // Error: message was already requeued once using notify()
                    // Don't do it again and disconnect client
                    ctx.stop()
                } else {
                    ctx.notify(CurrentServerState(connected_players, player_waiting, true));
                }
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

        if let BacklinkState::Linked(ref client_adapter_addr) = self.backlinked_state {
            let client_adapter_addr = client_adapter_addr.clone();
            match msg {
                PlayerPing => {
                    client_adapter_addr.do_send(ServerMessage::ServerPong);
                    ok
                }
                PlaceChip(column) => {
                    if let ClientLobbyState::InLobby {
                        player,
                        lobby: lobby_addr,
                    } = &self.lobby_state
                    {
                        lobby_addr.do_send(ClientLobbyMessageNamed {
                            sender: *player,
                            msg: ClientLobbyMessage::PlaceChip(column),
                        });
                        ok
                    } else {
                        client_adapter_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::NotInLobby)));
                        err
                    }
                }
                PlayAgainRequest => {
                    if let ClientLobbyState::InLobby {
                        player,
                        lobby: lobby_addr,
                    } = &self.lobby_state
                    {
                        lobby_addr.do_send(ClientLobbyMessageNamed {
                            sender: *player,
                            msg: ClientLobbyMessage::PlayAgainRequest,
                        });
                        ok
                    } else {
                        client_adapter_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::NotInLobby)));
                        err
                    }
                }
                Leaving => {
                    match &self.lobby_state {
                        ClientLobbyState::InLobby {
                            player,
                            lobby: lobby_addr,
                        } => {
                            // println!("ClientState: forwarding Leave message to lobby.");
                            if lobby_addr
                                .try_send(ClientLobbyMessageNamed {
                                    sender: *player,
                                    msg: ClientLobbyMessage::PlayerLeaving {
                                        reason: PlayerLeaveReason::Leave,
                                    },
                                })
                                .is_err()
                            {
                                // client_conn_addr
                                //     .do_send(ServerMessage::Error(Some(SrvMsgError::Internal)));
                                // TODO: Lobby is dead. Send okay or error here?
                            }
                        }
                        ClientLobbyState::Idle => {
                            client_adapter_addr
                                .do_send(ServerMessage::Error(Some(SrvMsgError::NotInLobby)));
                        }
                        ClientLobbyState::InLobbyWaitingForHost { .. } => {
                            client_adapter_addr
                                .do_send(ServerMessage::Error(Some(SrvMsgError::NotInLobby)));
                        }
                    }
                    self.lobby_state = ClientLobbyState::Idle;
                    ok
                }
                LobbyRequest(kind) => {
                    if let ClientLobbyState::Idle = &self.lobby_state {
                        self.lobby_mgr
                            .send(lobby_mgr::LobbyRequest::NewLobby(
                                ctx.address(),
                                self.maybe_user_info.clone().map(|u| u.id),
                                kind,
                            ))
                            .into_actor(self)
                            .then(move |res, act, ctx| {
                                act.received_lobby_request_response(res, ctx, client_adapter_addr);

                                fut::ready(())
                            })
                            .wait(ctx);
                        ok
                    } else {
                        client_adapter_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::AlreadyInLobby)));
                        err
                    }
                }
                LobbyJoin(id) => {
                    if let ClientLobbyState::Idle = &self.lobby_state {
                        self.lobby_mgr
                            .send(lobby_mgr::LobbyRequest::JoinLobby(
                                id,
                                ctx.address(),
                                self.maybe_user_info.clone().map(|u| u.id),
                                LobbyKind::Private,
                            ))
                            .into_actor(self)
                            .then(move |res, act, ctx| {
                                act.received_lobby_request_response(res, ctx, client_adapter_addr);
                                fut::ready(())
                            })
                            .wait(ctx);
                        ok
                    } else {
                        client_adapter_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::AlreadyInLobby)));
                        err
                    }
                }
                Login(session_token) => {
                    if let ClientLobbyState::InLobby {
                        player: _,
                        lobby: _,
                    } = self.lobby_state
                    {
                        client_adapter_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::AlreadyInLobby)));
                        return err;
                    }
                    if let Some(user_info) = self.maybe_user_info.clone() {
                        self.user_mgr
                            .do_send(user_mgr::msg::IntUserMgrMsg::StopPlaying(
                                user_info.id,
                                ctx.address(),
                            ));
                    }
                    self.user_mgr
                        .send(user_mgr::msg::StartPlaying {
                            session_token,
                            addr: ctx.address(),
                        })
                        .into_actor(self)
                        .then(move |res, act, _| {
                            if let Ok(maybe_id) = res {
                                match maybe_id {
                                    Ok(user) => {
                                        println!("Start playing! user: {:?}", user);
                                        act.maybe_user_info = Some(user);
                                        // act.user_mgr.do_send(IntUserMgrMsg::StartPlaying(id));
                                        client_adapter_addr.do_send(ServerMessage::Okay);
                                    }
                                    Err(srv_msg_err) => {
                                        client_adapter_addr
                                            .do_send(ServerMessage::Error(Some(srv_msg_err)));
                                    }
                                }
                            } else {
                                client_adapter_addr
                                    .do_send(ServerMessage::Error(Some(SrvMsgError::Internal)));
                            }
                            fut::ready(())
                        })
                        .wait(ctx);
                    ok
                }

                Logout => {
                    if let Some(user_info) = self.maybe_user_info.clone() {
                        self.user_mgr
                            .do_send(user_mgr::msg::IntUserMgrMsg::StopPlaying(
                                user_info.id,
                                ctx.address(),
                            ));
                    }
                    ok
                }

                BattleReq(friend_id) => {
                    if let ClientLobbyState::Idle = &self.lobby_state {
                        if let Some(user_info) = self.maybe_user_info.clone() {
                            self.user_mgr.do_send(user_mgr::msg::BattleReq {
                                sender_addr: ctx.address(),
                                sender_uid: user_info.id,
                                receiver_uid: friend_id,
                            });
                            ok
                        } else {
                            client_adapter_addr
                                .do_send(ServerMessage::Error(Some(SrvMsgError::NotLoggedIn)));
                            err
                        }
                    } else {
                        client_adapter_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::AlreadyInLobby)));
                        err
                    }
                }
                ChatMessage(msg) => {
                    if let ClientLobbyState::InLobby {
                        player,
                        lobby: lobby_addr,
                    } = &self.lobby_state
                    {
                        let username = if let Some(user_info) = self.maybe_user_info.clone() {
                            Some(user_info.username)
                        } else {
                            None
                        };
                        lobby_addr.do_send(ClientLobbyMessageNamed {
                            sender: *player,
                            msg: ClientLobbyMessage::ChatMessage(msg, username),
                        });
                    } else {
                        self.connection_mgr
                            .do_send(ConnectionManagerMsg::ChatMessage(self.id.clone(), msg));
                    }
                    client_adapter_addr.do_send(ServerMessage::Okay);
                    ok
                }
                ChatRead => {
                    if let ClientLobbyState::InLobby {
                        player,
                        lobby: lobby_addr,
                    } = &self.lobby_state
                    {
                        lobby_addr.do_send(ClientLobbyMessageNamed {
                            sender: *player,
                            msg: ClientLobbyMessage::ChatRead,
                        });
                    } else {
                        self.connection_mgr
                            .do_send(ConnectionManagerMsg::ChatRead(self.id.clone()));
                    }
                    ok
                }
                PlayerPong => todo!(),
                ReadyForGamePong => {
                    if let ClientLobbyState::InLobby { player: _, lobby } = &self.lobby_state {
                        lobby.do_send(LobbyMessage::ReceivedReadyForGamePong);
                        ok
                    } else {
                        err
                    }
                }
            }
        } else {
            err
        }
    }
}

impl Handler<ServerMessage> for ClientState {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: ServerMessage, _: &mut Self::Context) -> Self::Result {
        if let BacklinkState::Linked(ref adapter) = self.backlinked_state {
            adapter.do_send(msg);
            Ok(())
        } else {
            Err(())
        }
    }
}
impl Handler<ClientAdapterMsg> for ClientState {
    type Result = ();

    fn handle(&mut self, msg: ClientAdapterMsg, _: &mut Self::Context) -> Self::Result {
        if let BacklinkState::Linked(ref adapter) = self.backlinked_state {
            adapter.do_send(msg);
        }
    }
}

impl Message for ClientStateMessage {
    type Result = Result<(), ()>;
}

impl Handler<LobbyRequestResponseReady> for ClientState {
    type Result = ();

    fn handle(
        &mut self,
        _msg: LobbyRequestResponseReady,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        if let ClientLobbyState::InLobbyWaitingForHost { player, lobby } = self.lobby_state.clone()
        {
            self.lobby_state = ClientLobbyState::InLobby { player, lobby };
        }
    }
}

impl Actor for ClientState {
    type Context = Context<Self>;

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        // println!("ClientState: Stopping");

        if let Some(user_info) = self.maybe_user_info.clone() {
            self.user_mgr
                .do_send(user_mgr::msg::IntUserMgrMsg::StopPlaying(
                    user_info.id,
                    ctx.address(),
                ));
        }

        if let BacklinkState::Linked(client_adapter) = &self.backlinked_state {
            client_adapter.do_send(ClientAdapterMsg::Close);
        }
        Running::Stop
    }
}

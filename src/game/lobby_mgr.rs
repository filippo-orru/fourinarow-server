use super::client_adapter::ClientAdapter;
use super::client_state::{ClientState, ClientStateMessage};
use super::connection_mgr::{ConnectionManager, ConnectionManagerMsg};
use super::game_info::GameId;
use super::game_info::Player;
use super::lobby::*;
use super::msg::*;
use crate::{
    api::users::{
        user::{PlayedGameInfo, UserId},
        user_mgr,
    },
    logging::*,
};

use actix::*;
use std::collections::HashMap;
use std::time::Duration;

pub struct LobbyManager {
    open_lobby: Option<LobbyInfo>,
    open_lobby_map: LobbyMap,
    closed_lobby_map: LobbyMap,
    user_mgr: Addr<user_mgr::UserManager>,
    connection_mgr: Addr<ConnectionManager>,
    logger: Addr<Logger>,
}

impl LobbyManager {
    pub fn new(
        user_mgr: Addr<user_mgr::UserManager>,
        connection_mgr: Addr<ConnectionManager>,
        logger: Addr<Logger>,
    ) -> LobbyManager {
        LobbyManager {
            open_lobby: None,
            open_lobby_map: HashMap::new(),
            closed_lobby_map: HashMap::new(),
            user_mgr,
            connection_mgr,
            logger,
        }
    }

    fn create_lobby(
        &mut self,
        adapter_addr: Addr<ClientAdapter>,
        maybe_host_id: Option<UserId>,
        lobby_mgr_addr: Addr<LobbyManager>,
        user_mgr_addr: Addr<user_mgr::UserManager>,
        kind: LobbyKind,
    ) -> LobbyRequestResponseSuccess {
        let lobby_id = LobbyId::new();
        let game_id = GameId::generate(
            &self
                .open_lobby_map
                .keys()
                .clone()
                .chain(self.closed_lobby_map.keys().clone())
                .collect::<Vec<_>>(),
        );
        let lobby_addr = Lobby::new(
            lobby_id.clone(),
            game_id,
            lobby_mgr_addr,
            user_mgr_addr,
            self.logger.clone(),
            adapter_addr,
            maybe_host_id,
        )
        .start();
        match kind {
            LobbyKind::Public => {
                self.open_lobby = Some(LobbyInfo::new(
                    lobby_id.clone(),
                    game_id,
                    lobby_addr.clone(),
                    kind,
                ));
            }
            LobbyKind::Private => {
                self.open_lobby_map.insert(
                    game_id,
                    LobbyInfo::new(lobby_id.clone(), game_id, lobby_addr.clone(), kind),
                );
            }
        }

        LobbyRequestResponseSuccess {
            player: Player::One,
            game_id,
            lobby_addr,
        }
    }
}

pub type LobbyMap = HashMap<GameId, LobbyInfo>;

#[derive(Clone)]
pub struct LobbyInfo {
    lobby_id: LobbyId,
    game_id: GameId,
    addr: Addr<Lobby>,
    kind: LobbyKind,
    waiting_for_ready_response: Option<WaitingForReadyResponse>,
}
impl LobbyInfo {
    fn new(lobby_id: LobbyId, game_id: GameId, addr: Addr<Lobby>, kind: LobbyKind) -> LobbyInfo {
        LobbyInfo {
            lobby_id,
            game_id,
            addr,
            kind,
            waiting_for_ready_response: None,
        }
    }
}

#[derive(Clone)]
struct WaitingForReadyResponse {
    requesting_addr: Addr<ClientState>,
    adapter_addr: Addr<ClientAdapter>,
    maybe_user_id: Option<UserId>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LobbyKind {
    Private,
    Public,
}

pub enum LobbyRequest {
    NewLobby(
        Addr<ClientState>,
        Addr<ClientAdapter>,
        Option<UserId>,
        LobbyKind,
    ),
    JoinLobby(
        GameId,
        Addr<ClientState>,
        Addr<ClientAdapter>,
        Option<UserId>,
        LobbyKind,
    ),
}

impl Message for LobbyRequest {
    type Result = Result<LobbyRequestResponse, Option<SrvMsgError>>;
}

pub enum LobbyRequestResponse {
    Waiting(LobbyInfo),
    Success(LobbyRequestResponseSuccess),
}

pub struct LobbyRequestResponseSuccess {
    pub player: Player,
    pub game_id: GameId,
    pub lobby_addr: Addr<Lobby>,
}

impl Message for LobbyRequestResponseSuccess {
    type Result = ();
}

impl Handler<LobbyRequest> for LobbyManager {
    type Result = Result<LobbyRequestResponse, Option<SrvMsgError>>;
    fn handle(&mut self, request: LobbyRequest, ctx: &mut Self::Context) -> Self::Result {
        // println!("lobby_mgr: got req");
        match request {
            LobbyRequest::NewLobby(sender_addr, adapter_addr, maybe_user_id, kind) => {
                // println!("got new lobby req");
                match kind {
                    LobbyKind::Public => {
                        if let Some(mut open_lobby) = self.open_lobby.clone() {
                            open_lobby.addr.do_send(LobbyMessage::ReadyForBattlePing);
                            open_lobby.waiting_for_ready_response = Some(WaitingForReadyResponse {
                                requesting_addr: sender_addr,
                                adapter_addr,
                                maybe_user_id,
                            });
                            self.open_lobby = None;
                            Ok(LobbyRequestResponse::Waiting(open_lobby))
                        } else {
                            let response_success = self.create_lobby(
                                adapter_addr,
                                maybe_user_id,
                                ctx.address(),
                                self.user_mgr.clone(),
                                LobbyKind::Public,
                            );
                            self.connection_mgr
                                .do_send(ConnectionManagerMsg::Update(self.open_lobby.is_some()));

                            Ok(LobbyRequestResponse::Success(response_success))
                        }
                    }
                    LobbyKind::Private => {
                        let lobby_request_response = self.create_lobby(
                            adapter_addr.clone(),
                            maybe_user_id,
                            ctx.address(),
                            self.user_mgr.clone(),
                            LobbyKind::Private,
                        );

                        Ok(LobbyRequestResponse::Success(lobby_request_response))
                    }
                }
            }
            LobbyRequest::JoinLobby(id, sender_addr, adapter_addr, maybe_user_id, kind) => {
                // println!(
                //     "LobbyMgr: Requested to join lobby {} ({} active lobbies).",
                //     id,
                //     self.lobby_map.len()
                // );
                // print!("LobbyMgr: Joining lobby requested... ");
                if let Some(ref mut lobby_info) = self.open_lobby_map.get_mut(&id) {
                    if lobby_info.kind == kind {
                        lobby_info.addr.do_send(
                            PlayerJoined {
                                joining_addr: adapter_addr,
                                maybe_joining_uid: maybe_user_id,
                            },
                            //     ClientLobbyMessageNamed {
                            // msg:
                            //     sender: Player::Two,
                            // }
                        );

                        Ok(LobbyRequestResponse::Success(LobbyRequestResponseSuccess {
                            player: Player::Two,
                            game_id: id,
                            lobby_addr: lobby_info.addr.clone(),
                        }))
                    } else {
                        Err(Some(SrvMsgError::LobbyFull))
                    }
                } else {
                    sender_addr.do_send(ServerMessage::Error(Some(SrvMsgError::LobbyNotFound)));
                    // println!("LobbyMgr: Lobby {} not found!", id);
                    Err(Some(SrvMsgError::LobbyNotFound))
                }
            }
        }
    }
}

pub struct GetIsPlayerWaitingMsg;

impl Message for GetIsPlayerWaitingMsg {
    type Result = bool;
}

impl Handler<GetIsPlayerWaitingMsg> for LobbyManager {
    type Result = bool;

    fn handle(&mut self, _: GetIsPlayerWaitingMsg, _ctx: &mut Self::Context) -> Self::Result {
        self.open_lobby.is_some()
    }
}

pub enum LobbyManagerMsg {
    CloseLobbyMsg(GameId),
    PlayedGame(PlayedGameInfo),
    ReadyForBattleResponse(LobbyInfo),
    // Shutdown,
}
impl Message for LobbyManagerMsg {
    type Result = ();
}
impl Handler<LobbyManagerMsg> for LobbyManager {
    type Result = ();
    fn handle(&mut self, msg: LobbyManagerMsg, ctx: &mut Self::Context) -> Self::Result {
        use LobbyManagerMsg::*;
        match msg {
            CloseLobbyMsg(game_id) => {
                println!("LobbyMgr: Removed lobby {}", game_id);
                if let Some(LobbyInfo {
                    game_id: open_game_id,
                    ..
                }) = self.open_lobby
                {
                    if open_game_id == game_id {
                        self.open_lobby = None;
                    }
                }

                self.open_lobby_map.remove(&game_id);
                self.closed_lobby_map.remove(&game_id);
                self.connection_mgr
                    .do_send(ConnectionManagerMsg::Update(self.open_lobby.is_some()));
            }

            PlayedGame(game_info) => {
                self.user_mgr.do_send(user_mgr::msg::IntUserMgrMsg::Game(
                    user_mgr::msg::GameMsg::PlayedGame(game_info),
                ));
            } /*LobbyManagerMsg::Shutdown => {
            println!(
            "LobbyMgr: Shutting down ({} active lobbies).",
            self.lobby_map.len()
            );
            for (game_id, lobby_info) in self.lobby_map.drain() {
            println!("LobbyMgr: Sending close command to lobby {}", game_id);
            lobby_info.addr.do_send(LobbyMessage::LobbyClose);
            }
            ctx.stop();
            }*/
            ReadyForBattleResponse(lobby) => {
                if let Some(waiting_info) = lobby.waiting_for_ready_response.clone() {
                    lobby
                        .addr
                        .send(PlayerJoined {
                            joining_addr: waiting_info.adapter_addr.clone(),
                            maybe_joining_uid: waiting_info.maybe_user_id,
                        })
                        .into_actor(self)
                        .then(|_, _, _| fut::ready(()))
                        .wait(ctx);

                    self.closed_lobby_map.insert(lobby.game_id, lobby.clone());

                    self.connection_mgr
                        .do_send(ConnectionManagerMsg::Update(self.open_lobby.is_some()));

                    waiting_info
                        .requesting_addr
                        .do_send(LobbyRequestResponseSuccess {
                            player: Player::Two,
                            game_id: lobby.game_id,
                            lobby_addr: lobby.addr,
                        });
                }
            }
        }
    }
}

// pub struct GetInfo;
// impl Message for GetInfo {
//     type Result = Vec<LobbyInfo>;
// }
// impl Handler<GetInfo> for LobbyManager {
//     type Result = Vec<LobbyInfo>;
//     fn handle(&mut self, _: GetInfo, ctx: &mut Self::Context) -> Self::Result {
//         self.lobby_map.values().cloned().collect()
//     }
// }

// impl fmt::Debug for LobbyInfo {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         use fmt::Write;
//         write!(f, "{}", self.)
// }

pub struct BattleReq {
    pub sender: (Addr<ClientAdapter>, UserId),
    pub receiver: (Addr<ClientAdapter>, UserId),
}
impl Message for BattleReq {
    type Result = ();
}
impl Handler<BattleReq> for LobbyManager {
    type Result = ();
    fn handle(&mut self, msg: BattleReq, ctx: &mut Self::Context) {
        // println!("lobby_mgr: got battlereq");
        // ctx.notify(LobbyRequest::NewLobby(
        //     msg.sender.0.clone(),
        //     Some(msg.sender.1),
        //     LobbyKind::Private,
        // ))
        // // (
        // // )))
        // .into_actor(self)
        // .map(move |res, _, _| {
        //     if let Ok(Ok(response)) = res {
        //         msg.receiver
        //             .0
        //             .do_send(ServerMessage::BattleReq(msg.sender.1, response.game_id));

        //         msg.sender
        //             .0
        //             .do_send(ClientStateMessage::BattleReqJoinLobby(response.game_id));
        //     }
        // })
        // .wait(ctx);
        // msg.sender.0.do_send(ServerMessage::Okay);
        let lobby_info = self.create_lobby(
            msg.sender.0.clone(),
            Some(msg.sender.1),
            ctx.address(),
            self.user_mgr.clone(),
            LobbyKind::Private,
        );

        msg.sender.0.do_send(ClientStateMessage::BattleReqJoinLobby(
            lobby_info.lobby_addr,
        ));
        msg.receiver
            .0
            .do_send(ServerMessage::BattleReq(msg.sender.1, lobby_info.game_id));
        // msg.sender.0.do_send()
    }
}

impl Actor for LobbyManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.user_mgr
            .do_send(user_mgr::msg::IntUserMgrMsg::Backlink(ctx.address()));

        self.connection_mgr
            .do_send(ConnectionManagerMsg::Backlink(ctx.address()));
    }
}

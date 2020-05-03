use super::client_conn::ClientConnection;
use super::game_info::GameId;
use super::game_info::Player;
use super::lobby::*;
use super::msg::*;
use crate::api::users::{
    user::{PlayedGameInfo, UserId},
    user_mgr,
};

use actix::prelude::*;
use std::collections::HashMap;

pub struct LobbyManager {
    open_lobby: Option<LobbyInfo>,
    open_lobby_map: LobbyMap,
    closed_lobby_map: LobbyMap,
    user_mgr: Addr<user_mgr::UserManager>,
}
impl LobbyManager {
    pub fn new(user_mgr: Addr<user_mgr::UserManager>) -> LobbyManager {
        LobbyManager {
            open_lobby: None,
            open_lobby_map: HashMap::new(),
            closed_lobby_map: HashMap::new(),
            user_mgr,
        }
    }

    fn create_lobby(
        &mut self,
        host_addr: Addr<ClientConnection>,
        maybe_host_id: Option<UserId>,
        lobby_mgr_addr: Addr<LobbyManager>,
        user_mgr_addr: Addr<user_mgr::UserManager>,
        kind: LobbyKind,
    ) -> LobbyRequestResponse {
        let game_id = GameId::generate(
            &self
                .open_lobby_map
                .keys()
                .clone()
                .chain(self.closed_lobby_map.keys().clone())
                .collect::<Vec<_>>(),
        );
        let lobby_addr = Lobby::new(
            game_id,
            lobby_mgr_addr,
            user_mgr_addr,
            host_addr,
            maybe_host_id,
        )
        .start();
        match kind {
            LobbyKind::Public => {
                self.open_lobby = Some(LobbyInfo::new(game_id, lobby_addr.clone(), kind));
            }
            LobbyKind::Private => {
                self.open_lobby_map
                    .insert(game_id, LobbyInfo::new(game_id, lobby_addr.clone(), kind));
            }
        }

        // println!("LobbyMgr: {} lobbies after", self.lobby_map.len());
        // let _ = request.resp.send(Some(lobby_addr));
        LobbyRequestResponse {
            player: Player::One,
            game_id,
            lobby_addr,
        }
    }
}

pub type LobbyMap = HashMap<GameId, LobbyInfo>;

#[derive(Clone)]
pub struct LobbyInfo {
    game_id: GameId,
    addr: Addr<Lobby>,
    kind: LobbyKind,
}
impl LobbyInfo {
    fn new(game_id: GameId, addr: Addr<Lobby>, kind: LobbyKind) -> LobbyInfo {
        LobbyInfo {
            game_id,
            addr,
            kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LobbyKind {
    Private,
    Public,
}

pub enum LobbyRequest {
    NewLobby(Addr<ClientConnection>, Option<UserId>, LobbyKind),
    JoinLobby(GameId, Addr<ClientConnection>, Option<UserId>, LobbyKind),
}

pub struct LobbyRequestResponse {
    pub player: Player,
    pub game_id: GameId,
    pub lobby_addr: Addr<Lobby>,
}

impl Handler<LobbyRequest> for LobbyManager {
    type Result = Result<LobbyRequestResponse, ()>;
    fn handle(&mut self, request: LobbyRequest, ctx: &mut Self::Context) -> Self::Result {
        match request {
            LobbyRequest::NewLobby(host_addr, maybe_user_id, kind) => {
                if let LobbyKind::Public = kind {
                    let lobby_info = if let Some(open_lobby) = self.open_lobby.clone() {
                        self.open_lobby = None;
                        open_lobby
                            .addr
                            .send(PlayerJoined(host_addr.clone(), maybe_user_id))
                            .into_actor(self)
                            .then(|_, _, _| fut::ready(()))
                            .wait(ctx);
                        ctx.run_later(std::time::Duration::from_millis(0), move |_, _| {
                            host_addr.do_send(ServerMessage::OpponentJoining);
                        });
                        self.closed_lobby_map
                            .insert(open_lobby.game_id, open_lobby.clone());

                        LobbyRequestResponse {
                            player: Player::Two,
                            game_id: open_lobby.game_id,
                            lobby_addr: open_lobby.addr,
                        }
                    } else {
                        host_addr.do_send(ServerMessage::Okay);
                        self.create_lobby(
                            host_addr,
                            maybe_user_id,
                            ctx.address(),
                            self.user_mgr.clone(),
                            LobbyKind::Public,
                        )
                    };
                    Ok(lobby_info)
                } else {
                    let lobby_info = self.create_lobby(
                        host_addr.clone(),
                        maybe_user_id,
                        ctx.address(),
                        self.user_mgr.clone(),
                        LobbyKind::Private,
                    );

                    host_addr.do_send(ServerMessage::LobbyResponse(lobby_info.game_id));
                    Ok(lobby_info)
                }
            }
            LobbyRequest::JoinLobby(id, client_addr, maybe_user_id, kind) => {
                // println!(
                //     "LobbyMgr: Requested to join lobby {} ({} active lobbies).",
                //     id,
                //     self.lobby_map.len()
                // );
                // print!("LobbyMgr: Joining lobby requested... ");
                if let Some(ref mut lobby_info) = self.open_lobby_map.get_mut(&id) {
                    if lobby_info.kind == kind {
                        lobby_info.addr.do_send(
                            PlayerJoined(client_addr, maybe_user_id),
                            //     ClientLobbyMessageNamed {
                            // msg:
                            //     sender: Player::Two,
                            // }
                        );

                        Ok(LobbyRequestResponse {
                            player: Player::Two,
                            game_id: id,
                            lobby_addr: lobby_info.addr.clone(),
                        })
                    } else {
                        client_addr.do_send(ServerMessage::Error(Some(SrvMsgError::LobbyFull)));
                        Err(())
                    }
                } else {
                    client_addr.do_send(ServerMessage::Error(Some(SrvMsgError::LobbyNotFound)));
                    // println!("LobbyMgr: Lobby {} not found!", id);
                    Err(())
                }
            }
        }
    }
}

pub enum LobbyManagerMsg {
    CloseLobbyMsg(GameId),
    PlayedGame(PlayedGameInfo),
    // Shutdown,
}
impl Message for LobbyManagerMsg {
    type Result = ();
}
impl Handler<LobbyManagerMsg> for LobbyManager {
    type Result = ();
    fn handle(&mut self, msg: LobbyManagerMsg, _ctx: &mut Self::Context) -> Self::Result {
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
                        return;
                    }
                }

                self.open_lobby_map.remove(&game_id);
                self.closed_lobby_map.remove(&game_id);
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

impl Actor for LobbyManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.user_mgr
            .do_send(user_mgr::msg::IntUserMgrMsg::Backlink(ctx.address()));
    }
}
impl Message for LobbyRequest {
    type Result = Result<LobbyRequestResponse, ()>;
}

// impl fmt::Debug for LobbyInfo {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         use fmt::Write;
//         write!(f, "{}", self.)
// }

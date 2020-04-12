use super::client_conn::ClientConnection;
use super::game_info::GameId;
use super::game_info::Player;
use super::lobby::*;
use super::msg::*;

use actix::prelude::*;
use std::collections::HashMap;

pub struct LobbyManager {
    lobby_map: LobbyMap,
}
impl LobbyManager {
    pub fn new() -> LobbyManager {
        LobbyManager {
            lobby_map: HashMap::new(),
        }
    }
}

pub type LobbyMap = HashMap<GameId, LobbyInfo>;
#[derive(Clone)]
pub struct LobbyInfo {
    addr: Addr<Lobby>,
    open: bool,
}
impl LobbyInfo {
    fn new(addr: Addr<Lobby>) -> LobbyInfo {
        LobbyInfo { addr, open: true }
    }
}

pub enum LobbyRequest {
    NewLobby(Addr<ClientConnection>),
    JoinLobby(GameId, Addr<ClientConnection>),
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
            LobbyRequest::NewLobby(host_addr) => {
                // println!(
                //     "LobbyMgr: Requested lobby ({} active lobbies).",
                //     self.lobby_map.len()
                // );
                // println!("LobbyMgr: New lobby requested");
                let game_id = GameId::generate(&self.lobby_map);
                host_addr.do_send(ServerMessage::LobbyResponse(game_id));
                // println!("LobbyMgr: {} lobbies before", self.lobby_map.len());
                let lobby_addr = Lobby::new(game_id, ctx.address(), host_addr).start();
                self.lobby_map
                    .insert(game_id, LobbyInfo::new(lobby_addr.clone()));
                // println!("LobbyMgr: {} lobbies after", self.lobby_map.len());
                // let _ = request.resp.send(Some(lobby_addr));
                Ok(LobbyRequestResponse {
                    player: Player::One,
                    game_id,
                    lobby_addr,
                })
            }
            LobbyRequest::JoinLobby(id, client_addr) => {
                // println!(
                //     "LobbyMgr: Requested to join lobby {} ({} active lobbies).",
                //     id,
                //     self.lobby_map.len()
                // );
                // print!("LobbyMgr: Joining lobby requested... ");
                if let Some(ref mut lobby_info) = self.lobby_map.get_mut(&id) {
                    if lobby_info.open {
                        lobby_info.open = false;
                        lobby_info.addr.do_send(ClientLobbyMessageNamed {
                            sender: Player::Two,
                            msg: ClientLobbyMessage::PlayerJoined(client_addr),
                        });

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
    // Shutdown,
}
impl Message for LobbyManagerMsg {
    type Result = ();
}
impl Handler<LobbyManagerMsg> for LobbyManager {
    type Result = ();
    fn handle(&mut self, lm_msg: LobbyManagerMsg, _ctx: &mut Self::Context) -> Self::Result {
        match lm_msg {
            LobbyManagerMsg::CloseLobbyMsg(game_id) => {
                println!("LobbyMgr: Removed lobby {}", game_id);
                self.lobby_map.remove(&game_id);
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
}
impl Message for LobbyRequest {
    type Result = Result<LobbyRequestResponse, ()>;
}

// impl fmt::Debug for LobbyInfo {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         use fmt::Write;
//         write!(f, "{}", self.)
// }

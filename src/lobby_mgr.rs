use crate::client_conn::ClientConnection;
use crate::game::GameId;
use crate::game::Player;
use crate::lobby::*;
use crate::msg::*;

use actix::prelude::*;
use std::collections::HashMap;

pub type LobbyMap = HashMap<GameId, Addr<Lobby>>;

pub struct LobbyManager {
    map: LobbyMap,
}
impl LobbyManager {
    pub fn new() -> LobbyManager {
        LobbyManager {
            map: HashMap::new(),
        }
    }
}

impl Actor for LobbyManager {
    type Context = Context<Self>;
}

// pub struct LobbyRequestMsg {
//     pub resp: oneshot::Sender<Option<Addr<Lobby>>>,
//     pub msg: LobbyRequest,
// }

pub enum LobbyRequest {
    NewLobby(Addr<ClientConnection>),
    JoinLobby(GameId, Addr<ClientConnection>),
}
impl Message for LobbyRequest {
    type Result = Result<LobbyRequestResponse, ()>;
}

pub enum LobbyManagerMsg {
    CloseLobbyMsg(GameId),
    Shutdown,
}
impl Message for LobbyManagerMsg {
    type Result = ();
}

impl Handler<LobbyRequest> for LobbyManager {
    type Result = Result<LobbyRequestResponse, ()>;
    fn handle(&mut self, request: LobbyRequest, ctx: &mut Self::Context) -> Self::Result {
        match request {
            LobbyRequest::NewLobby(host_addr) => {
                // println!("LobbyMgr: New lobby requested");
                let game_id = GameId::generate(&self.map);
                let lobby_addr = Lobby::new(game_id, ctx.address(), host_addr).start();
                self.map.insert(game_id, lobby_addr.clone());
                // let _ = request.resp.send(Some(lobby_addr));
                Ok(LobbyRequestResponse {
                    player: Player::One,
                    game_id,
                    lobby_addr,
                })
            }
            LobbyRequest::JoinLobby(id, client_addr) => {
                // print!("LobbyMgr: Joining lobby requested... ");
                if let Some(lobby_addr) = self.map.remove(&id) {
                    lobby_addr.do_send(ClientLobbyMessageNamed {
                        sender: Player::Two,
                        msg: ClientLobbyMessage::PlayerJoined(client_addr),
                    });
                    // println!("Ok!");

                    Ok(LobbyRequestResponse {
                        player: Player::Two,
                        game_id: id,
                        lobby_addr,
                    })
                } else {
                    client_addr.do_send(ServerMessage::Error(Some(SrvMsgError::LobbyNotFound)));
                    println!("Lobby {} not found!", id);
                    Err(())
                }
            }
        }
    }
}

impl Handler<LobbyManagerMsg> for LobbyManager {
    type Result = ();
    fn handle(&mut self, lm_msg: LobbyManagerMsg, ctx: &mut Self::Context) -> Self::Result {
        match lm_msg {
            LobbyManagerMsg::CloseLobbyMsg(game_id) => {
                self.map.remove(&game_id);
            }
            LobbyManagerMsg::Shutdown => {
                for lobby_addr in self.map.values() {
                    lobby_addr.do_send(LobbyMessage::Shutdown);
                }
                ctx.stop();
            }
        }
    }
}

pub struct LobbyRequestResponse {
    pub player: Player,
    pub game_id: GameId,
    pub lobby_addr: Addr<Lobby>,
}

use crate::client_conn::ClientConnection;
use crate::client_state::ClientStateMessage;
use crate::game::{GameId, GameInfo, Player};
use crate::lobby_mgr::{LobbyManager, LobbyManagerMsg};
use crate::msg::*;

use actix::*;
use std::time::Duration;

pub enum LobbyState {
    OnePlayer(Addr<ClientConnection>),
    TwoPlayers(GameInfo, Addr<ClientConnection>, Addr<ClientConnection>),
}

// #[derive(Debug, Clone, Copy)]
pub struct ClientLobbyMessageNamed {
    pub sender: Player,
    pub msg: ClientLobbyMessage,
}

pub enum ClientLobbyMessage {
    PlayerJoined(Addr<ClientConnection>),
    PlayerLeaving,
    PlaceChip(usize),
}

impl Message for ClientLobbyMessageNamed {
    type Result = Result<(), ()>;
}

pub struct Lobby {
    game_id: GameId,
    lobby_mgr_addr: Addr<LobbyManager>,
    game_state: LobbyState,
}

impl Handler<ClientLobbyMessageNamed> for Lobby {
    type Result = Result<(), ()>;
    fn handle(
        &mut self,
        msg_named: ClientLobbyMessageNamed,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        use ClientLobbyMessage::*;
        match msg_named.msg {
            PlayerJoined(client_addr) => {
                if let LobbyState::OnePlayer(ref host_addr) = self.game_state {
                    host_addr.do_send(ServerMessage::OpponentJoining);
                    client_addr.do_send(ServerMessage::Okay);
                    self.game_state =
                        LobbyState::TwoPlayers(GameInfo::new(), host_addr.clone(), client_addr);
                    ctx.notify_later(LobbyMessage::GameStart, Duration::from_secs(2));
                    Ok(())
                } else {
                    Err(())
                }
            }
            PlayerLeaving => {
                self.lobby_mgr_addr
                    .do_send(LobbyManagerMsg::CloseLobbyMsg(self.game_id));
                match &self.game_state {
                    LobbyState::TwoPlayers(_, host_addr, client_addr) => {
                        let leaving_addr = msg_named.sender.select(host_addr, client_addr);
                        let other_addr = msg_named.sender.other().select(host_addr, client_addr);
                        other_addr.do_send(ServerMessage::OpponentLeaving);
                        other_addr.do_send(ClientStateMessage::OpponentLeaving);
                        leaving_addr.do_send(ServerMessage::Okay);
                    }
                    LobbyState::OnePlayer(host_addr) => {
                        host_addr.do_send(ServerMessage::Okay);
                    }
                }
                ctx.stop();
                Ok(())
            }

            PlaceChip(column) => match self.game_state {
                LobbyState::TwoPlayers(ref mut game_info, ref host_addr, ref client_addr) => {
                    match game_info.place_chip(column, msg_named.sender) {
                        Ok(()) => {
                            let placing_addr = msg_named.sender.select(host_addr, client_addr);
                            placing_addr.do_send(ServerMessage::Okay);
                            let other_addr =
                                msg_named.sender.other().select(host_addr, client_addr);
                            other_addr.do_send(ServerMessage::PlaceChip(column));

                            Ok(())
                        }
                        Err(srvmsgerr) => {
                            let placing_addr = msg_named.sender.select(host_addr, client_addr);
                            placing_addr.do_send(ServerMessage::Error(srvmsgerr));
                            Err(())
                        }
                    }
                }
                LobbyState::OnePlayer(ref host_addr) => {
                    host_addr.do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                    Err(())
                }
            },
        }
    }
}

pub enum LobbyMessage {
    GameStart,
    Shutdown,
}

impl Message for LobbyMessage {
    type Result = Result<(), ()>;
}

impl Handler<LobbyMessage> for Lobby {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: LobbyMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            LobbyMessage::Shutdown => {
                match self.game_state {
                    LobbyState::TwoPlayers(_, ref host_addr, ref client_addr) => {
                        host_addr.do_send(ServerMessage::LobbyClosing);
                        client_addr.do_send(ServerMessage::LobbyClosing);
                    }
                    LobbyState::OnePlayer(ref host_addr) => {
                        host_addr.do_send(ServerMessage::LobbyClosing)
                    }
                }
                ctx.stop();
                Ok(())
            }
            LobbyMessage::GameStart => {
                if let LobbyState::TwoPlayers(game_state, host_addr, client_addr) = &self.game_state
                {
                    // game_state.turn =
                    host_addr.do_send(ServerMessage::GameStart(game_state.turn == Player::One));
                    client_addr.do_send(ServerMessage::GameStart(game_state.turn == Player::Two));
                    Ok(())
                } else {
                    Err(())
                }
            }
        }
    }
}

impl Lobby {
    pub fn new(
        id: GameId,
        lobby_mgr_addr: Addr<LobbyManager>,
        host_addr: Addr<ClientConnection>,
    ) -> Lobby {
        Lobby {
            game_id: id,
            lobby_mgr_addr,
            game_state: LobbyState::OnePlayer(host_addr),
        }
    }
}

impl Actor for Lobby {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        if let LobbyState::OnePlayer(host_addr) = &self.game_state {
            host_addr.do_send(ServerMessage::LobbyResponse(self.game_id));
        }
    }
}

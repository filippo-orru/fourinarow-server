use super::client_conn::ClientConnection;
use super::client_state::ClientStateMessage;
use super::game_info::{GameId, GameInfo, Player};
use super::lobby_mgr::{LobbyManager, LobbyManagerMsg};
use super::msg::*;

use actix::*;
use std::time::{Duration, Instant};

const LOBBY_TIMEOUT_S: u64 = 5 * 60;

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
    PlayAgainRequest,
    PlaceChip(usize),
}

impl Message for ClientLobbyMessageNamed {
    type Result = Result<(), ()>;
}

pub struct Lobby {
    game_id: GameId,
    lobby_mgr_addr: Addr<LobbyManager>,
    game_state: LobbyState,
    last_hb: Instant,
}

impl Handler<ClientLobbyMessageNamed> for Lobby {
    type Result = Result<(), ()>;
    fn handle(
        &mut self,
        msg_named: ClientLobbyMessageNamed,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        self.last_hb = Instant::now();

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
                        other_addr.do_send(ClientStateMessage::Reset);
                        other_addr.do_send(ServerMessage::OpponentLeaving);
                        leaving_addr.do_send(ServerMessage::Okay);
                    }
                    LobbyState::OnePlayer(host_addr) => {
                        host_addr.do_send(ServerMessage::Okay);
                    }
                }
                ctx.stop();
                Ok(())
            }
            PlayAgainRequest => {
                match &self.game_state {
                    LobbyState::TwoPlayers(game_info, host_addr, client_addr) => {
                        let requesting_addr = msg_named.sender.select(host_addr, client_addr);
                        if let Some(winner_info) = &game_info.winner {
                            if let Some(already_requested) = winner_info.requesting_rematch {
                                if already_requested == msg_named.sender {
                                    // sender requested again, but okay :shrug:
                                    requesting_addr.do_send(ServerMessage::Okay);
                                } else {
                                    // both have now requested -> rematch
                                    requesting_addr.do_send(ServerMessage::Okay);
                                    ctx.notify(LobbyMessage::GameStart);
                                }
                            }
                        } else {
                            // game not over yet
                            requesting_addr
                                .do_send(ServerMessage::Error(Some(SrvMsgError::GameNotOver)));
                        }
                    }
                    LobbyState::OnePlayer(host_addr) => {
                        host_addr.do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                    }
                }
                Ok(())
            }
            PlaceChip(column) => match self.game_state {
                LobbyState::TwoPlayers(ref mut game_info, ref host_addr, ref client_addr) => {
                    match game_info.place_chip(column, msg_named.sender) {
                        Ok(maybe_winner) => {
                            let placing_addr = msg_named.sender.select(host_addr, client_addr);
                            placing_addr.do_send(ServerMessage::Okay);
                            let other_addr =
                                msg_named.sender.other().select(host_addr, client_addr);
                            other_addr.do_send(ServerMessage::PlaceChip(column));
                            if let Some(winner) = maybe_winner {
                                placing_addr
                                    .do_send(ServerMessage::GameOver(msg_named.sender == winner));
                                other_addr.do_send(ServerMessage::GameOver(
                                    msg_named.sender.other() == winner,
                                ));
                            }

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
    LobbyClose,
}

impl Handler<LobbyMessage> for Lobby {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: LobbyMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            LobbyMessage::LobbyClose => {
                match self.game_state {
                    LobbyState::TwoPlayers(_, ref host_addr, ref client_addr) => {
                        host_addr.do_send(ClientStateMessage::Reset);
                        host_addr.do_send(ServerMessage::LobbyClosing);

                        client_addr.do_send(ClientStateMessage::Reset);
                        client_addr.do_send(ServerMessage::LobbyClosing);
                    }
                    LobbyState::OnePlayer(ref host_addr) => {
                        host_addr.do_send(ClientStateMessage::Reset);
                        host_addr.do_send(ServerMessage::LobbyClosing);
                    }
                }
                ctx.stop();
                Ok(())
            }
            LobbyMessage::GameStart => {
                if let LobbyState::TwoPlayers(game_state, host_addr, client_addr) =
                    &mut self.game_state
                {
                    game_state.reset();
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
        game_id: GameId,
        lobby_mgr_addr: Addr<LobbyManager>,
        host_addr: Addr<ClientConnection>,
    ) -> Lobby {
        Lobby {
            game_id,
            lobby_mgr_addr,
            game_state: LobbyState::OnePlayer(host_addr),
            last_hb: Instant::now(),
        }
    }
}

impl Actor for Lobby {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(
            Duration::from_secs(5),
            |act: &mut Self, ctx: &mut Self::Context| {
                if act.last_hb.elapsed() > Duration::from_secs(LOBBY_TIMEOUT_S) {
                    println!("Lobby: Timed out.");
                    ctx.notify(LobbyMessage::LobbyClose);
                }
            },
        );
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        println!("Lobby ({}): closing.", self.game_id);
        Running::Stop
    }
}

impl Message for LobbyMessage {
    type Result = Result<(), ()>;
}

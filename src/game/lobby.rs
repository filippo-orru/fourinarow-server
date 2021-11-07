use super::client_state::{ClientState, ClientStateMessage};
use super::game_info::{GameId, GameInfo, GameType, Player};
use super::lobby_mgr::{LobbyManager, LobbyManagerMsg, LobbyRequestResponseReady};
use super::msg::*;
use crate::{
    api::users::{
        user::{PlayedGameInfo, UserId},
        user_mgr,
    },
    logging::*,
};

use actix::*;
// use futures;
use std::time::{Duration, Instant};

const LOBBY_TIMEOUT_S: u64 = 30 * 60; // 30 minutes
const GAME_START_DELAY_S: u64 = 2;
const GAME_READY_RESPONSE_TIMEOUT_MS: u64 = 5000; // TODO: 1 second

enum LobbyState {
    OnePlayer {
        host_info: LobbyPlayerInfo,
    },
    TwoPlayersWaitingForPing {
        host_info: LobbyPlayerInfo,
        joined_info: LobbyPlayerInfo,
        timeout_handle: SpawnHandle,
    },
    TwoPlayers {
        game_oid: GameOId,
        game_type: GameType,
        host_addr: Addr<ClientState>,
        joined_addr: Addr<ClientState>,
    },
}

#[derive(Clone)]
struct LobbyPlayerInfo {
    addr: Addr<ClientState>,
    maybe_uid: Option<UserId>,
}

pub struct ClientLobbyMessageNamed {
    pub sender: Player,
    pub msg: ClientLobbyMessage,
}

pub enum ClientLobbyMessage {
    PlayerLeaving { reason: PlayerLeaveReason },
    PlayAgainRequest,
    PlaceChip(usize),
    ChatMessage(String, Option<String>), // content, sender
    ChatRead,
}

pub enum PlayerLeaveReason {
    Leave,
    Disconnect,
}

impl Message for ClientLobbyMessageNamed {
    type Result = Result<(), ()>;
}

pub struct Lobby {
    lobby_id: LobbyId,
    game_id: GameId,
    #[allow(dead_code)]
    user_mgr: Addr<user_mgr::UserManager>,
    lobby_mgr: Addr<LobbyManager>,
    logger: Addr<Logger>,
    lobby_state: LobbyState,
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
            PlayerLeaving {
                reason: leave_reason,
            } => {
                self.lobby_mgr
                    .do_send(LobbyManagerMsg::CloseLobbyMsg(self.game_id));
                self.logger.do_send(LobbyLogEvent::LobbyClosed {
                    id: self.lobby_id.clone(),
                });

                fn send_messages(
                    logger: &Addr<Logger>,
                    leave_reason: PlayerLeaveReason,
                    game_oid: Option<GameOId>,
                    sender: Player,
                    host_addr: &Addr<ClientState>,
                    joined_addr: &Addr<ClientState>,
                ) {
                    if let Some(game_oid) = game_oid {
                        logger.do_send(GameLogEvent::EndGame {
                            id: game_oid,
                            reason: match leave_reason {
                                PlayerLeaveReason::Leave => GameEndReason::PlayerLeft,
                                PlayerLeaveReason::Disconnect => GameEndReason::PlayerDisconnected,
                            },
                        });
                    }
                    let other_addr = sender.other().select(host_addr, joined_addr);
                    other_addr.do_send(ClientStateMessage::Reset);
                    other_addr.do_send(ServerMessage::OpponentLeaving);
                }

                match &self.lobby_state {
                    LobbyState::TwoPlayers {
                        game_oid,
                        game_type: _,
                        host_addr,
                        joined_addr,
                    } => {
                        send_messages(
                            &self.logger,
                            leave_reason,
                            Some(game_oid.clone()),
                            msg_named.sender,
                            host_addr,
                            joined_addr,
                        );
                    }
                    LobbyState::TwoPlayersWaitingForPing {
                        host_info,
                        joined_info,
                        ..
                    } => {
                        send_messages(
                            &self.logger,
                            leave_reason,
                            None,
                            msg_named.sender,
                            &host_info.addr,
                            &joined_info.addr,
                        );
                    }
                    LobbyState::OnePlayer { .. } => {}
                }
                ctx.stop();
                Ok(())
            }
            PlayAgainRequest => {
                match &mut self.lobby_state {
                    LobbyState::TwoPlayers {
                        game_oid: _,
                        game_type,
                        host_addr,
                        joined_addr,
                    } => {
                        let requesting_addr = msg_named.sender.select(host_addr, joined_addr);
                        match game_type {
                            GameType::Registered(game_info, _, _)
                            | GameType::Anonymous(game_info) => {
                                if let Some(winner_info) = &mut game_info.winner {
                                    if let Some(already_requested) = winner_info.requesting_rematch
                                    {
                                        if already_requested != msg_named.sender {
                                            // both have now requested -> rematch
                                            ctx.notify(LobbyMessage::GameStart);
                                        } else {
                                            // sender requested again, but okay :shrug:
                                            // requesting_addr.do_send(ServerMessage::Okay);
                                        }
                                    } else {
                                        winner_info.requesting_rematch = Some(msg_named.sender);
                                    }
                                } else {
                                    // game not over yet
                                    requesting_addr.do_send(ServerMessage::Error(Some(
                                        SrvMsgError::GameNotOver,
                                    )));
                                }
                            }
                        }
                        Ok(())
                    }
                    LobbyState::TwoPlayersWaitingForPing {
                        host_info,
                        joined_info,
                        ..
                    } => {
                        host_info
                            .addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                        joined_info
                            .addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                        Err(())
                    }
                    LobbyState::OnePlayer { host_info } => {
                        host_info
                            .addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                        Err(())
                    }
                }
            }
            PlaceChip(column) => match self.lobby_state {
                LobbyState::TwoPlayers {
                    ref game_oid,
                    ref mut game_type,
                    ref host_addr,
                    ref joined_addr,
                } => match game_type {
                    GameType::Registered(game_info, _, _) | GameType::Anonymous(game_info) => {
                        match game_info.place_chip(column, msg_named.sender) {
                            Ok(maybe_winner) => {
                                let placing_addr = msg_named.sender.select(host_addr, joined_addr);

                                let other_addr =
                                    msg_named.sender.other().select(host_addr, joined_addr);
                                other_addr.do_send(ServerMessage::PlaceChip(column));
                                if let Some(winner) = maybe_winner {
                                    placing_addr.do_send(ServerMessage::GameOver(
                                        msg_named.sender == winner,
                                    ));
                                    other_addr.do_send(ServerMessage::GameOver(
                                        msg_named.sender.other() == winner,
                                    ));
                                    self.logger.do_send(GameLogEvent::EndGame {
                                        id: game_oid.clone(),
                                        reason: GameEndReason::Regular,
                                    });
                                    if let GameType::Registered(_, host_id, joined_id) = game_type {
                                        let (winner, loser) =
                                            winner.select_both(*host_id, *joined_id);
                                        println!("{} won against {}", winner, loser);
                                        let game_info = PlayedGameInfo::new(winner, loser);
                                        self.lobby_mgr
                                            .do_send(LobbyManagerMsg::PlayedGame(game_info));
                                    }
                                }

                                Ok(())
                            }
                            Err(srvmsgerr) => {
                                let placing_addr = msg_named.sender.select(host_addr, joined_addr);
                                placing_addr.do_send(ServerMessage::Error(srvmsgerr));
                                Err(())
                            }
                        }
                    }
                },
                LobbyState::TwoPlayersWaitingForPing {
                    ref host_info,
                    ref joined_info,
                    ..
                } => {
                    host_info
                        .addr
                        .do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                    joined_info
                        .addr
                        .do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                    Err(())
                }
                LobbyState::OnePlayer { ref host_info } => {
                    host_info
                        .addr
                        .do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                    Err(())
                }
            },
            ChatMessage(msg, sender) => match self.lobby_state {
                LobbyState::TwoPlayers {
                    game_oid: _,
                    game_type: _,
                    ref host_addr,
                    ref joined_addr,
                } => {
                    let msg_recipient = msg_named.sender.other().select(host_addr, joined_addr);
                    msg_recipient.do_send(ServerMessage::ChatMessage(false, msg, sender));
                    Ok(())
                }
                _ => Err(()),
            },
            ChatRead => match self.lobby_state {
                LobbyState::TwoPlayers {
                    game_oid: _,
                    game_type: _,
                    ref host_addr,
                    ref joined_addr,
                } => {
                    let msg_recipient = msg_named.sender.other().select(host_addr, joined_addr);
                    msg_recipient.do_send(ServerMessage::ChatRead(false));
                    Ok(())
                }
                _ => Err(()),
            },
        }
    }
}

pub enum LobbyMessage {
    ReceivedReadyForGamePong,
    GameStart,
    LobbyClose,
    PlayerJoined {
        joined_addr: Addr<ClientState>,
        maybe_uid: Option<UserId>,
    },
}

impl Handler<LobbyMessage> for Lobby {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: LobbyMessage, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            LobbyMessage::LobbyClose => {
                ctx.stop();
                Ok(())
            }
            LobbyMessage::GameStart => {
                if let LobbyState::TwoPlayers {
                    game_oid: _,
                    game_type,
                    ref host_addr,
                    ref joined_addr,
                } = &mut self.lobby_state
                {
                    match game_type {
                        GameType::Registered(game_info, _, _) | GameType::Anonymous(game_info) => {
                            game_info.reset();
                        }
                    }
                    match game_type {
                        GameType::Anonymous(game_info) => {
                            host_addr.do_send(ServerMessage::GameStart(
                                game_info.turn == Player::One,
                                None,
                            ));
                            joined_addr.do_send(ServerMessage::GameStart(
                                game_info.turn == Player::Two,
                                None,
                            ));
                        }
                        GameType::Registered(game_info, host_id, joined_id) => {
                            // let user_mgr = self.user_mgr.clone();
                            host_addr.do_send(ServerMessage::GameStart(
                                game_info.turn == Player::One,
                                Some(joined_id.to_string()),
                            ));
                            joined_addr.do_send(ServerMessage::GameStart(
                                game_info.turn == Player::Two,
                                Some(host_id.to_string()),
                            ));
                        }
                    }

                    Ok(())
                } else {
                    Err(())
                }
            }
            LobbyMessage::ReceivedReadyForGamePong => {
                if let LobbyState::TwoPlayersWaitingForPing {
                    host_info,
                    joined_info,
                    timeout_handle,
                } = &mut self.lobby_state
                {
                    ctx.cancel_future(*timeout_handle);
                    joined_info.addr.do_send(LobbyRequestResponseReady);
                    host_info.addr.do_send(ServerMessage::OpponentJoining);
                    joined_info.addr.do_send(ServerMessage::OpponentJoining);
                    self.lobby_state = if let (Some(host_id), Some(joined_id)) =
                        (host_info.maybe_uid, joined_info.maybe_uid)
                    {
                        let game_info = GameInfo::new();
                        let game_oid = GameOId::new();
                        self.logger.do_send(GameLogEvent::StartGame {
                            id: game_oid.clone(),
                            ranked: true,
                        });
                        LobbyState::TwoPlayers {
                            game_oid: game_oid,
                            game_type: GameType::Registered(game_info, host_id, joined_id),
                            host_addr: host_info.addr.clone(),
                            joined_addr: joined_info.addr.clone(),
                        }
                    } else {
                        let game_info = GameInfo::new();
                        let game_oid = GameOId::new();
                        self.logger.do_send(GameLogEvent::StartGame {
                            id: game_oid.clone(),
                            ranked: false,
                        });
                        LobbyState::TwoPlayers {
                            game_oid: game_oid,
                            game_type: GameType::Anonymous(game_info),
                            host_addr: host_info.addr.clone(),
                            joined_addr: joined_info.addr.clone(),
                        }
                    };
                    ctx.notify_later(
                        LobbyMessage::GameStart,
                        Duration::from_secs(GAME_START_DELAY_S),
                    );

                    Ok(())
                } else {
                    Err(())
                }
            }
            LobbyMessage::PlayerJoined {
                joined_addr,
                maybe_uid,
            } => {
                if let LobbyState::OnePlayer { ref host_info } = self.lobby_state {
                    // In case other player does not respond
                    let joined_info = LobbyPlayerInfo {
                        addr: joined_addr.clone(),
                        maybe_uid,
                    };

                    let timeout_handle = ctx.run_later(
                        Duration::from_millis(GAME_READY_RESPONSE_TIMEOUT_MS),
                        move |act, ctx| {
                            if let LobbyState::TwoPlayersWaitingForPing { .. } = act.lobby_state {
                                ctx.stop();
                            }
                        },
                    );
                    host_info.addr.do_send(ServerMessage::ReadyForGamePing);
                    self.lobby_state = LobbyState::TwoPlayersWaitingForPing {
                        host_info: host_info.clone(),
                        joined_info,
                        timeout_handle,
                    };

                    Ok(())
                } else {
                    joined_addr.do_send(ServerMessage::OpponentJoining);
                    Err(())
                }
            }
        }
    }
}

impl Lobby {
    pub fn new(
        lobby_id: LobbyId,
        game_id: GameId,
        lobby_mgr: Addr<LobbyManager>,
        user_mgr: Addr<user_mgr::UserManager>,
        logger: Addr<Logger>,
        host_state: Addr<ClientState>,
        maybe_host_id: Option<UserId>,
    ) -> Lobby {
        Lobby {
            lobby_id,
            game_id,
            lobby_mgr,
            user_mgr,
            logger,
            lobby_state: LobbyState::OnePlayer {
                host_info: LobbyPlayerInfo {
                    addr: host_state,
                    maybe_uid: maybe_host_id,
                },
            },
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
                    println!("Lobby ({}): Timed out.", act.game_id);
                    ctx.notify(LobbyMessage::LobbyClose);
                }
            },
        );
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        println!("Lobby ({}): closing.", self.game_id);

        match self.lobby_state {
            LobbyState::TwoPlayers {
                ref host_addr,
                ref joined_addr,
                ..
            } => {
                host_addr.do_send(ClientStateMessage::Reset);
                host_addr.do_send(ServerMessage::LobbyClosing);

                joined_addr.do_send(ClientStateMessage::Reset);
                joined_addr.do_send(ServerMessage::LobbyClosing);
            }
            LobbyState::TwoPlayersWaitingForPing {
                ref host_info,
                ref joined_info,
                ..
            } => {
                host_info.addr.do_send(ClientStateMessage::Reset);
                host_info.addr.do_send(ServerMessage::LobbyClosing);

                joined_info.addr.do_send(ClientStateMessage::Reset);
                joined_info.addr.do_send(ServerMessage::LobbyClosing);
            }
            LobbyState::OnePlayer { ref host_info } => {
                host_info.addr.do_send(ClientStateMessage::Reset);
                host_info.addr.do_send(ServerMessage::LobbyClosing);
            }
        }
        Running::Stop
    }
}

impl Message for LobbyMessage {
    type Result = Result<(), ()>;
}

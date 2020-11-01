use super::client_conn::ClientConnection;
use super::client_state::ClientStateMessage;
use super::game_info::{GameId, GameInfo, GameType, Player};
use super::lobby_mgr::{LobbyManager, LobbyManagerMsg};
use super::msg::*;
use crate::api::users::{
    user::{PlayedGameInfo, UserId},
    user_mgr,
};

use actix::*;
// use futures;
use std::time::{Duration, Instant};

const LOBBY_TIMEOUT_S: u64 = 30 * 60; // 30 Minutes

pub enum LobbyState {
    OnePlayer(Addr<ClientConnection>, Option<UserId>),
    TwoPlayers(GameType, Addr<ClientConnection>, Addr<ClientConnection>),
}

// #[derive(Debug, Clone, Copy)]
pub struct ClientLobbyMessageNamed {
    pub sender: Player,
    pub msg: ClientLobbyMessage,
}

pub struct PlayerJoined(pub Addr<ClientConnection>, pub Option<UserId>);
impl Message for PlayerJoined {
    type Result = Result<(), ()>;
}
impl Handler<PlayerJoined> for Lobby {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: PlayerJoined, ctx: &mut Self::Context) -> Self::Result {
        if let LobbyState::OnePlayer(ref host_addr, maybe_host_id) = self.game_state {
            msg.0.do_send(ServerMessage::Okay);
            host_addr.do_send(ServerMessage::OpponentJoining);
            self.game_state = if let (Some(host_id), Some(joined_id)) = (maybe_host_id, msg.1) {
                LobbyState::TwoPlayers(
                    GameType::Registered(GameInfo::new(), host_id, joined_id),
                    host_addr.clone(),
                    msg.0,
                )
            } else {
                LobbyState::TwoPlayers(
                    GameType::Anonymous(GameInfo::new()),
                    host_addr.clone(),
                    msg.0,
                )
            };
            ctx.notify_later(LobbyMessage::GameStart, Duration::from_secs(2));
            Ok(())
        } else {
            Err(())
        }
    }
}

pub enum ClientLobbyMessage {
    PlayerLeaving,
    PlayAgainRequest,
    PlaceChip(usize),
}

impl Message for ClientLobbyMessageNamed {
    type Result = Result<(), ()>;
}

pub struct Lobby {
    game_id: GameId,
    #[allow(dead_code)]
    user_mgr: Addr<user_mgr::UserManager>,
    lobby_mgr: Addr<LobbyManager>,
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
            PlayerLeaving => {
                self.lobby_mgr
                    .do_send(LobbyManagerMsg::CloseLobbyMsg(self.game_id));
                match &self.game_state {
                    LobbyState::TwoPlayers(_, host_addr, client_addr) => {
                        let leaving_addr = msg_named.sender.select(host_addr, client_addr);
                        let other_addr = msg_named.sender.other().select(host_addr, client_addr);
                        other_addr.do_send(ClientStateMessage::Reset);
                        other_addr.do_send(ServerMessage::OpponentLeaving);
                        leaving_addr.do_send(ServerMessage::Okay);
                    }
                    LobbyState::OnePlayer(host_addr, _) => {
                        host_addr.do_send(ServerMessage::Okay);
                    }
                }
                ctx.stop();
                Ok(())
            }
            PlayAgainRequest => {
                match &mut self.game_state {
                    LobbyState::TwoPlayers(game_type, host_addr, client_addr) => {
                        let requesting_addr = msg_named.sender.select(host_addr, client_addr);
                        match game_type {
                            GameType::Registered(game_info, _, _)
                            | GameType::Anonymous(game_info) => {
                                if let Some(winner_info) = &mut game_info.winner {
                                    if let Some(already_requested) = winner_info.requesting_rematch
                                    {
                                        requesting_addr.do_send(ServerMessage::Okay);
                                        if already_requested != msg_named.sender {
                                            // both have now requested -> rematch
                                            ctx.notify(LobbyMessage::GameStart);
                                        } else {
                                            // sender requested again, but okay :shrug:
                                            // requesting_addr.do_send(ServerMessage::Okay);
                                        }
                                    } else {
                                        winner_info.requesting_rematch = Some(msg_named.sender);
                                        requesting_addr.do_send(ServerMessage::Okay);
                                    }
                                } else {
                                    // game not over yet
                                    requesting_addr.do_send(ServerMessage::Error(Some(
                                        SrvMsgError::GameNotOver,
                                    )));
                                }
                            }
                        }
                    }
                    LobbyState::OnePlayer(host_addr, _) => {
                        host_addr.do_send(ServerMessage::Error(Some(SrvMsgError::GameNotStarted)));
                    }
                }
                Ok(())
            }
            PlaceChip(column) => match self.game_state {
                LobbyState::TwoPlayers(ref mut game_type, ref host_addr, ref client_addr) => {
                    match game_type {
                        GameType::Registered(game_info, _, _) | GameType::Anonymous(game_info) => {
                            match game_info.place_chip(column, msg_named.sender) {
                                Ok(maybe_winner) => {
                                    let placing_addr =
                                        msg_named.sender.select(host_addr, client_addr);
                                    placing_addr.do_send(ServerMessage::Okay);
                                    let other_addr =
                                        msg_named.sender.other().select(host_addr, client_addr);
                                    other_addr.do_send(ServerMessage::PlaceChip(column));
                                    if let Some(winner) = maybe_winner {
                                        placing_addr.do_send(ServerMessage::GameOver(
                                            msg_named.sender == winner,
                                        ));
                                        other_addr.do_send(ServerMessage::GameOver(
                                            msg_named.sender.other() == winner,
                                        ));
                                        if let GameType::Registered(_, host_id, joined_id) =
                                            game_type
                                        {
                                            let (winner, loser) = winner
                                                .select_both(host_id.clone(), joined_id.clone());
                                            println!("{} won against {}", winner, loser);
                                            let game_info = PlayedGameInfo::new(winner, loser);
                                            self.lobby_mgr
                                                .do_send(LobbyManagerMsg::PlayedGame(game_info));
                                        }
                                    }

                                    Ok(())
                                }
                                Err(srvmsgerr) => {
                                    let placing_addr =
                                        msg_named.sender.select(host_addr, client_addr);
                                    placing_addr.do_send(ServerMessage::Error(srvmsgerr));
                                    Err(())
                                }
                            }
                        }
                    }
                }
                LobbyState::OnePlayer(ref host_addr, _) => {
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
                ctx.stop();
                Ok(())
            }
            LobbyMessage::GameStart => {
                if let LobbyState::TwoPlayers(game_type, host_addr, joined_addr) =
                    &mut self.game_state
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
                            // println!("before async");
                            // || {
                            // async {
                            // if let (Ok(Some(host_user)), Ok(Some(joined_user))) = (
                            // user_mgr
                            //     .send(user_mgr::msg::GetUser(UserIdent::Id(*host_id)))
                            //     .into_actor(self)
                            //     .wait(ctx),
                            // user_mgr
                            //     .send(user_mgr::msg::GetUser(UserIdent::Id(*joined_id)))
                            //     .into_actor(self)
                            //     .wait(ctx),
                            // ) {
                            // if let (Ok(Some(host_user)), Ok(Some(joined_user))) =
                            //     // futures::executor::block_on(async {
                            //     (
                            //     ctx.wait(
                            //         user_mgr
                            //             .send(user_mgr::msg::GetUser(UserIdent::Id(*host_id)))
                            //             .into_actor(self),
                            //     ),
                            //     ctx.wait(
                            //         user_mgr
                            //             .send(user_mgr::msg::GetUser(UserIdent::Id(*joined_id)))
                            //             .into_actor(self),
                            //     ),
                            // ) {
                            // user_mgr
                            //     .send(user_mgr::msg::GetUser(UserIdent::Id(*host_id)))
                            //     .into_actor(self)
                            //     .then(|host_res: Result<Option<PublicUser>, _>, act, ctx_inner| {
                            //         user_mgr
                            //             .send(user_mgr::msg::GetUser(UserIdent::Id(*joined_id)))
                            //             // .send(user_mgr::msg::GetUser(UserIdent::Id(*joined_id)))
                            //             .into_actor(act)
                            //             .then(|joined_res: Result<Option<PublicUser>, _>, _, _| {
                            //                 if let (Ok(Some(host_user)), Ok(Some(joined_user))) =
                            //                     (host_res, joined_res)
                            //                 {
                            //                     host_addr.clone().do_send(
                            //                         ServerMessage::GameStart(
                            //                             game_info.turn == Player::One,
                            //                             Some(host_user.username),
                            //                         ),
                            //                     );
                            //                     joined_addr.clone().do_send(
                            //                         ServerMessage::GameStart(
                            //                             game_info.turn == Player::Two,
                            //                             Some(joined_user.username),
                            //                         ),
                            //                     );
                            //                 } else {
                            //                     println!("Game not starting :(");
                            //                     host_addr.clone().do_send(ServerMessage::Error(
                            //                         Some(SrvMsgError::IncorrectCredentials),
                            //                     ));
                            //                     joined_addr.clone().do_send(ServerMessage::Error(
                            //                         Some(SrvMsgError::IncorrectCredentials),
                            //                     ));
                            //                     ctx.stop();
                            //                 }
                            //                 fut::ready(())
                            //             })
                            //             .wait(ctx_inner);
                            //         fut::ready(())
                            //     })
                            //     .wait(ctx);
                            // }
                            // .into_actor(self)
                            // .wait(ctx);
                            // println!("after async");
                            // }
                            // fut::ready(())
                            //         })
                            //         .wait(ctx);
                            // })
                            // .wait(ctx);
                            // return Ok(());
                        }
                    }

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
        lobby_mgr: Addr<LobbyManager>,
        user_mgr: Addr<user_mgr::UserManager>,
        host_addr: Addr<ClientConnection>,
        maybe_host_id: Option<UserId>,
    ) -> Lobby {
        Lobby {
            game_id,
            lobby_mgr,
            user_mgr,
            game_state: LobbyState::OnePlayer(host_addr, maybe_host_id),
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
        match self.game_state {
            LobbyState::TwoPlayers(_, ref host_addr, ref client_addr) => {
                host_addr.do_send(ClientStateMessage::Reset);
                host_addr.do_send(ServerMessage::LobbyClosing);

                client_addr.do_send(ClientStateMessage::Reset);
                client_addr.do_send(ServerMessage::LobbyClosing);
            }
            LobbyState::OnePlayer(ref host_addr, _) => {
                host_addr.do_send(ClientStateMessage::Reset);
                host_addr.do_send(ServerMessage::LobbyClosing);
            }
        }
        Running::Stop
    }
}

impl Message for LobbyMessage {
    type Result = Result<(), ()>;
}

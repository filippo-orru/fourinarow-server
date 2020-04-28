use super::client_state::*;
use super::lobby_mgr::LobbyManager;
use super::msg::*;
use crate::api::users::user_manager::UserManager;

use actix::*;
use actix_web_actors::ws;

use std::time::{Duration, Instant};

const HB_INTERVAL: u64 = 2;
const HB_TIMEOUT: u64 = 8;

pub struct ClientConnection {
    hb: Instant,
    client_state_addr: Addr<ClientState>,
}

impl ClientConnection {
    pub fn new(lobby_mgr: Addr<LobbyManager>, user_mgr: Addr<UserManager>) -> ClientConnection {
        // client_conn_addr: Addr<ClientConnection>,
        let client_state_addr = ClientState::new(lobby_mgr, user_mgr).start();
        ClientConnection {
            hb: Instant::now(),
            client_state_addr,
        }
    }
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Duration::from_secs(HB_INTERVAL), |act, ctx| {
            //&mut WsClientConnection
            //: &mut ws::WebsocketContext<WsClientConnection>
            if act.hb.elapsed().as_secs() >= HB_TIMEOUT {
                // println!("Client timed out");
                act.client_state_addr.do_send(ClientStateMessage::Close);

                ctx.stop();

                return;
            }
            ctx.ping(b"");
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ClientConnection {
    fn handle(&mut self, msg_res: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let ws_msg = match msg_res {
            Err(e) => {
                println!("ClientConn: Protocoll Error ({})", e);
                ctx.stop();
                return;
            }
            Ok(m) => m,
        };
        match ws_msg {
            ws::Message::Ping(ws_msg) => {
                // println!("<ping>");
                self.hb = Instant::now();
                ctx.pong(&ws_msg);
            }
            ws::Message::Pong(_) => {
                // println!("<pong>");
                self.hb = Instant::now();
            }
            ws::Message::Binary(_) => println!("ClientConn: Unexpected binary"),
            ws::Message::Close(_) => {
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                println!("ClientConn: got continuation (??) Stopping");
                ctx.stop();
            }
            ws::Message::Nop => (),
            ws::Message::Text(str_msg) => {
                print!(">> {:?}", str_msg);
                if let Some(player_msg) = PlayerMessage::parse(&str_msg) {
                    println!();
                    self.client_state_addr
                        .send(player_msg)
                        .into_actor(self)
                        .then(|msg_res, _, ctx: &mut Self::Context| {
                            if msg_res.is_err() {
                                ctx.notify(ServerMessage::Error(Some(SrvMsgError::Internal)));
                                println!("ClientConn: Failed to send message to client state");
                                ctx.stop();
                            }
                            fut::ready(())
                        })
                        .wait(ctx);
                } else {
                    ctx.notify(ServerMessage::Error(Some(SrvMsgError::InvalidMessage)));
                    println!("  ## -> Invalid message!");
                }
            }
        }
    }
}

impl Handler<ServerMessage> for ClientConnection {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: ServerMessage, ctx: &mut Self::Context) -> Self::Result {
        let msg_str = msg.serialize();
        println!("<< {:?}", msg_str);
        ctx.text(msg_str);
        Ok(())
    }
}

impl Handler<ClientStateMessage> for ClientConnection {
    type Result = Result<(), ()>;
    fn handle(&mut self, msg: ClientStateMessage, _: &mut Self::Context) -> Self::Result {
        self.client_state_addr.do_send(msg);
        Ok(())
    }
}

impl Actor for ClientConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        self.client_state_addr
            .do_send(ClientStateMessage::BackLink(ctx.address()));

        // self.game_state = GameState::WaitingInLobby(PlayerInfo(Addr::new(AddressSender::)));
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        println!("ClientConn: Stopping");
        self.client_state_addr.do_send(ClientStateMessage::Close);
        Running::Stop
    }
}

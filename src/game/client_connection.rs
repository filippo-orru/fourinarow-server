use super::{
    client_adapter::{ClientAdapter, ClientMsgString, MIN_VERSION},
    connection_mgr::{ConnectionManager, NewAdapterAdresses, SessionToken},
    msg::{HelloOut, PlayerMessage},
};
use super::{connection_mgr::ConnectionManagerMsg, lobby_mgr::LobbyManager};
use crate::api::users::user_mgr::UserManager;
use crate::game::msg::HelloIn;

use actix::*;
use actix_web_actors::ws;

use std::time::{Duration, Instant};

const HB_INTERVAL: u64 = 2;
const HB_TIMEOUT: u64 = 8;

pub struct ClientConnection {
    hb: Instant,
    connection_state: ClientAdapterConnectionState,
    connection_mgr: Addr<ConnectionManager>,
    lobby_mgr: Addr<LobbyManager>,
    user_mgr: Addr<UserManager>,
}

enum ClientAdapterConnectionState {
    Connected(SessionToken, Addr<ClientAdapter>),
    ConnectedLegacy(SessionToken, Addr<ClientAdapter>), // Old clients bypass the new reliability layer & adapter and sent straight to state
    Pending,
    NotConnected,
}

impl ClientConnection {
    pub fn new(
        lobby_mgr: Addr<LobbyManager>,
        user_mgr: Addr<UserManager>,
        connection_mgr: Addr<ConnectionManager>,
    ) -> ClientConnection {
        ClientConnection {
            hb: Instant::now(),
            connection_mgr,
            lobby_mgr,
            user_mgr,
            connection_state: ClientAdapterConnectionState::NotConnected,
        }
    }

    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Duration::from_secs(HB_INTERVAL), |act, ctx| {
            //&mut WsClientConnection
            //: &mut ws::WebsocketContext<WsClientConnection>
            if act.hb.elapsed().as_secs() >= HB_TIMEOUT {
                // println!("Client timed out");
                // act.client_state_addr.do_send(ClientStateMessage::Close);
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }

    fn text<T: Into<String>>(&self, ctx: &mut ws::WebsocketContext<Self>, msg: T) {
        let id = if let ClientAdapterConnectionState::Connected(id, _) = &self.connection_state {
            &id[0..3]
        } else {
            ""
        };
        let msg_str = msg.into();
        println!("{}<< {:?}", id, msg_str);
        ctx.text(msg_str);
    }

    fn received_text(&mut self, ctx: &mut ws::WebsocketContext<Self>, str_msg: String) {
        let id = if let ClientAdapterConnectionState::Connected(id, _) = &self.connection_state {
            &id[0..3]
        } else {
            ""
        };
        if str_msg.to_lowercase().contains("login") {
            println!(">> LOGIN:***:***");
        } else {
            println!("{}>> {:?}", id, str_msg);
        }

        match &self.connection_state {
            ClientAdapterConnectionState::NotConnected => {
                if let Some(hello) = HelloIn::parse(&str_msg) {
                    if hello.protocol_version < MIN_VERSION {
                        self.text(ctx, HelloOut::OutDated.serialize());
                        ctx.stop();
                        return;
                    }

                    if let Some(session_token) = hello.maybe_session_token {
                        self.connection_mgr
                            .do_send(ConnectionManagerMsg::RequestAdapterExisting(
                                NewAdapterAdresses {
                                    client_conn: ctx.address(),
                                    lobby_mgr: self.lobby_mgr.clone(),
                                    user_mgr: self.user_mgr.clone(),
                                },
                                session_token.to_string(),
                            ));
                    } else {
                        self.connection_mgr
                            .do_send(ConnectionManagerMsg::RequestAdapterNew(
                                NewAdapterAdresses {
                                    client_conn: ctx.address(),
                                    lobby_mgr: self.lobby_mgr.clone(),
                                    user_mgr: self.user_mgr.clone(),
                                },
                            ))
                    }
                    self.connection_state = ClientAdapterConnectionState::Pending;
                } else {
                    if PlayerMessage::parse(&str_msg).is_some() {
                        println!("  \\_LEGACY");
                        self.connection_mgr
                            .do_send(ConnectionManagerMsg::RequestAdapterLegacy(
                                NewAdapterAdresses {
                                    client_conn: ctx.address(),
                                    lobby_mgr: self.lobby_mgr.clone(),
                                    user_mgr: self.user_mgr.clone(),
                                },
                                str_msg.clone(),
                            ));
                    // let state_addr = ClientState::new(
                    //     ConnectionManager::generate_session_token(),
                    //     self.lobby_mgr.clone(),
                    //     self.user_mgr.clone(),
                    //     self.connection_mgr.clone(),
                    // )
                    // .start();
                    // state_addr.do_send(ClientStateMessage::BackLinkLegacy(ctx.address()));
                    // self.connection_state =
                    //     ClientAdapterConnectionState::ConnectedLegacy(state_addr.clone());
                    // state_addr.do_send(player_msg);

                    // TODO ^^
                    } else {
                        self.text(ctx, "NOT_CONNECTED");
                    }
                }
            }
            ClientAdapterConnectionState::ConnectedLegacy(_, adapter_addr) => {
                adapter_addr.do_send(ClientMsgString(str_msg));
                // if let Some(player_msg) = PlayerMessage::parse(&str_msg) {
                //     } else {
                //         self.text(ctx, "ERR");
                //         ctx.stop();
                //     }
            }
            ClientAdapterConnectionState::Connected(_, adapter_addr) => {
                adapter_addr.do_send(ClientMsgString(str_msg));
            }
            ClientAdapterConnectionState::Pending => {
                self.text(ctx, "WAIT");
            }
        }
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
                self.hb = Instant::now();
                ctx.pong(&ws_msg);
            }
            ws::Message::Pong(_) => {
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
            ws::Message::Text(str_msg) => self.received_text(ctx, str_msg),
        }
    }
}

impl Handler<ClientMsgString> for ClientConnection {
    type Result = ();

    fn handle(&mut self, msg: ClientMsgString, ctx: &mut Self::Context) -> Self::Result {
        self.text(ctx, msg);
    }
}

pub struct ClientConnnectionMsg {
    pub session_token: SessionToken,
    pub client_adapter: Addr<ClientAdapter>,
    pub connection_type: ConnectionType,
}

pub enum ConnectionType {
    Reliable { is_new: bool }, // is_new
    Legacy,
}

impl Message for ClientConnnectionMsg {
    type Result = ();
}

impl Handler<ClientConnnectionMsg> for ClientConnection {
    type Result = ();

    fn handle(&mut self, msg: ClientConnnectionMsg, ctx: &mut Self::Context) -> Self::Result {
        match msg.connection_type {
            ConnectionType::Reliable { is_new } => {
                self.connection_state = ClientAdapterConnectionState::Connected(
                    msg.session_token.clone(),
                    msg.client_adapter,
                );
                self.text(ctx, HelloOut::Ok(msg.session_token, is_new).serialize());
            }
            ConnectionType::Legacy => {
                self.connection_state = ClientAdapterConnectionState::ConnectedLegacy(
                    msg.session_token,
                    msg.client_adapter,
                );
            }
        }
    }
}

impl Actor for ClientConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        match &self.connection_state {
            ClientAdapterConnectionState::Connected(id, _) => {
                self.connection_mgr
                    .do_send(ConnectionManagerMsg::Disconnect {
                        session_token: id.clone(),
                        is_legacy: false,
                    });
            }
            ClientAdapterConnectionState::ConnectedLegacy(id, _) => {
                self.connection_mgr
                    .do_send(ConnectionManagerMsg::Disconnect {
                        session_token: id.clone(),
                        is_legacy: true,
                    });
            }
            _ => {}
        }
        Running::Stop
    }
}

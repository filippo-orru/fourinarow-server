use super::{
    client_adapter::{ClientAdapter, ClientMsgString},
    connection_mgr::{ConnectionManager, Identifier},
};
use super::{connection_mgr::ConnectionManagerMsg, lobby_mgr::LobbyManager};
use crate::api::users::user_mgr::UserManager;

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
    Connected(Identifier, Addr<ClientAdapter>),
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
            ws::Message::Text(str_msg) => {
                let id = if let ClientAdapterConnectionState::Connected(id, _) =
                    &self.connection_state
                {
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
                        if str_msg == "NEW" {
                            self.connection_mgr
                                .do_send(ConnectionManagerMsg::RequestAdapterNew(
                                    ctx.address(),
                                    self.lobby_mgr.clone(),
                                    self.user_mgr.clone(),
                                ))
                        } else if str_msg.starts_with("REQ::") {
                            let identifier = if let Some(id) = str_msg.split("::").nth(1) {
                                id
                            } else {
                                return;
                            };
                            if identifier.len() != 32 {
                                return;
                            }
                            self.connection_mgr.do_send(
                                ConnectionManagerMsg::RequestAdapterExisting(
                                    ctx.address(),
                                    identifier.to_string(),
                                ),
                            );
                            self.connection_state = ClientAdapterConnectionState::Pending;
                        }
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
    }
}

impl Handler<ClientMsgString> for ClientConnection {
    type Result = ();

    fn handle(&mut self, msg: ClientMsgString, ctx: &mut Self::Context) -> Self::Result {
        self.text(ctx, msg);
    }
}

pub enum ClientConnnectionMsg {
    Link(Identifier, Addr<ClientAdapter>),
    NotFound,
}

impl Message for ClientConnnectionMsg {
    type Result = ();
}

impl Handler<ClientConnnectionMsg> for ClientConnection {
    type Result = ();

    fn handle(&mut self, msg: ClientConnnectionMsg, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ClientConnnectionMsg::Link(id, addr) => {
                self.connection_state = ClientAdapterConnectionState::Connected(id.clone(), addr);
                self.text(ctx, &format!("READY::{}", id));
            }
            ClientConnnectionMsg::NotFound => {
                self.connection_state = ClientAdapterConnectionState::NotConnected;
                self.text(ctx, "NOT_FOUND");
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
        if let ClientAdapterConnectionState::Connected(id, _) = &self.connection_state {
            self.connection_mgr
                .do_send(ConnectionManagerMsg::Disconnect(id.clone()));
        }
        Running::Stop
    }
}
